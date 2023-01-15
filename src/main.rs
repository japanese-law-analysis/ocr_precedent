//! 判例のPDFをOCRでテキストに変換するプログラム
//!
//! requires:
//! - [tesseract](https://github.com/tesseract-ocr/tesseract)
//! - tesseract-ocr-jpn
//! - ImageMagick
//! - poppler-utils
//!
//! ---
//! [MIT License](https://github.com/japanese-law-analysis/ocr_precedent/blob/master/LICENSE)
//! (c) 2021 Naoki Kaneko (a.k.a. "puripuri2100")
//!

use anyhow::{anyhow, Result};
use clap::Parser;
use regex::Regex;
use serde_json::Value;
use std::path::Path;
use tokio::{
  self,
  fs::{self, *},
  io::AsyncWriteExt,
  process::Command,
};
use tokio_stream::StreamExt;

async fn download_pdf(path: &str, url: &str) -> Result<()> {
  let response = reqwest::get(url).await?;
  let bytes = response.bytes().await?;
  let mut f = File::create(path).await?;
  f.write_all(&bytes).await?;
  f.flush().await?;
  Ok(())
}

async fn get_pdf_page_size(path: &str) -> Result<usize> {
  let output = Command::new("pdfinfo").arg(path).output().await?;
  let text = String::from_utf8_lossy(&output.stdout);
  let re = Regex::new(r"Pages:\s*(?P<page_size>[\d]+)")?;
  let str = re
    .find(&text)
    .ok_or_else(|| anyhow!("ページ数取得失敗"))?
    .as_str();
  let page_size = re
    .captures(str)
    .ok_or_else(|| anyhow!("ページ数取得失敗(capture)"))?
    .name("page_size")
    .expect("")
    .as_str()
    .parse::<usize>()?;
  Ok(page_size)
}

async fn convert_pdf(name: &str) -> Option<String> {
  let output = Command::new("pdftoppm")
    .arg("-jpeg")
    .arg(format!("{name}.pdf"))
    .arg(name)
    .output()
    .await
    .ok();
  output.and_then(|output| {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.as_ref().is_empty() {
      None
    } else {
      Some(stderr.to_string())
    }
  })
}

/// エラーがあった場合はエラーを取得する
async fn crop_img(file_path: &str) -> Option<String> {
  let output = Command::new("convert")
    .arg("-crop")
    .arg("1000x1475+150+150")
    .arg(file_path)
    .arg(file_path)
    .output()
    .await
    .ok();
  output.and_then(|output| {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.as_ref().is_empty() {
      None
    } else {
      Some(stderr.to_string())
    }
  })
}

async fn ocr_img(name: &str) -> Option<String> {
  let output = Command::new("tesseract")
    .arg(format!("{name}.jpg"))
    .arg(name)
    .arg("-l")
    .arg("jpn")
    .output()
    .await
    .ok();
  output.and_then(|output| {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.as_ref().is_empty() {
      None
    } else {
      Some(stderr.to_string())
    }
  })
}

async fn join_ocr_text(text: &str) -> String {
  let mut s = String::new();
  let mut line_stream = tokio_stream::iter(text.lines());
  let mut is_null_line = false;
  while let Some(line) = line_stream.next().await {
    let text = line.trim();
    if text.is_empty() {
      is_null_line = true
    } else {
      if is_null_line {
        s.push('\n');
      }
      s.push_str(text);
      is_null_line = false;
    }
  }
  s
}

async fn join_ocr_file(file_path_lst: &[String], output_path: &str) -> Result<()> {
  let mut s = String::new();
  let mut stream = tokio_stream::iter(file_path_lst);
  while let Some(file_path) = stream.next().await {
    let file_contents = fs::read_to_string(file_path).await?;
    s.push_str(file_contents.trim());
  }
  let s = join_ocr_text(&s).await;
  let mut output = File::create(output_path).await?;
  output.write_all(s.as_bytes()).await?;
  output.flush().await?;
  Ok(())
}

async fn download_and_ocr(name: &str, url: &str, tmp_name: &str, is_downloads: bool) -> Result<()> {
  let file_name = format!("{tmp_name}/{name}");
  let file_path_pdf = format!("{file_name}.pdf");
  let file_path_txt = format!("{name}.txt");
  if is_downloads {
    println!("[START] downloads: {url}");
    download_pdf(&file_path_pdf, url).await?;
    println!("[END] downloads: {url}");
  }
  let pdf_size = get_pdf_page_size(&file_path_pdf).await?;
  let err_msg_opt = convert_pdf(&file_name).await;
  if let Some(err_msg) = err_msg_opt {
    println!("convert err({name}): {err_msg}");
  }
  let mut stream = tokio_stream::iter(1..=pdf_size);
  while let Some(page_num) = stream.next().await {
    let file_path = format!("{file_name}-{page_num}.jpg");
    let err_msg_opt = crop_img(&file_path).await;
    if let Some(err_msg) = err_msg_opt {
      println!("crop err({name}): {err_msg}");
    }
    let err_msg_opt = ocr_img(&format!("{file_name}-{page_num}")).await;
    if let Some(err_msg) = err_msg_opt {
      println!("ocr err({name}): {err_msg}");
    }
  }
  let txt_path_lst = (1..=pdf_size)
    .map(|i| format!("{file_name}-{i}.txt"))
    .collect::<Vec<_>>();
  join_ocr_file(&txt_path_lst, &file_path_txt).await?;
  Ok(())
}

#[derive(Clone, Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
  /// 判例のリストがあるJSONファイルへのpath
  #[arg(short, long)]
  input: String,
  /// 一時フォルダのpath
  #[arg(short, long, default_value_t=String::from("tmp"))]
  tmp: String,
  /// PDFのキャッシュを作成しない場合に付けるフラグ
  #[arg(long, default_value_t = false)]
  do_not_use_cache: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = Args::parse();
  let tmp_name = &args.tmp;
  fs::create_dir_all(tmp_name).await?;
  let input_file_path = &args.input;
  let input_json = fs::read_to_string(input_file_path).await?;
  let input_json_lst: Vec<Value> = serde_json::from_str(&input_json)?;
  let mut json_stream = tokio_stream::iter(input_json_lst);
  while let Some(v) = json_stream.next().await {
    let case_number = v
      .get("case_number")
      .and_then(|v| v.as_str())
      .ok_or_else(|| anyhow!("case_numberフィールドが無い"))?;
    println!("case_number: {case_number}");
    let date = v
      .get("date")
      .ok_or_else(|| anyhow!("dateフィールドが無い"))?;
    let year = date
      .get("year")
      .and_then(|v| v.as_u64())
      .ok_or_else(|| anyhow!("date/yearフィールドが無い"))?;
    let month = date
      .get("month")
      .and_then(|v| v.as_u64())
      .ok_or_else(|| anyhow!("date/monthフィールドが無い"))?;
    let day = date
      .get("day")
      .and_then(|v| v.as_u64())
      .ok_or_else(|| anyhow!("date/dayフィールドが無い"))?;
    let name = format!("{case_number}_{year}_{month}_{day}");
    let tmp_pdf_file_path = format!("{tmp_name}/{name}.pdf");
    let path = Path::new(&tmp_pdf_file_path);
    let is_downloads = if !args.do_not_use_cache {
      // キャッシュを使うので、ファイルが無かったら動かす
      !path.exists()
    } else {
      // キャッシュを使わないので常に実行
      true
    };
    let url = v
      .get("full_pdf_link")
      .and_then(|v| v.as_str())
      .ok_or_else(|| anyhow!("full_pdf_linkフィールドが無い"))?;
    println!("[START] write: {name}");
    println!("[START] url: {url}");
    download_and_ocr(&name, url, tmp_name, is_downloads).await?;
    println!("[END] write: {name}");
  }
  Ok(())
}
