//! 判例のPDFをOCRでテキストに変換するプログラム
//!
//! [listup_precedent](https://github.com/japanese-law-analysis/listup_precedent)で作成した裁判例の一覧をもとに、[裁判所のHP](https://www.courts.go.jp)から判決文PDFファイルをダウンロードしてテキストに直すソフトウェアです。
//!
//! # Install
//! requires:
//! - [tesseract](https://github.com/tesseract-ocr/tesseract)
//! - tesseract-ocr-jpn
//! - ImageMagick
//! - poppler-utils
//!
//! ubuntu:
//! ```sh
//! sudo apt update
//! sudo apt install tesseract-ocr libtesseract-dev tesseract-ocr-jpn imagemagick poppler-utils
//! cargo install --git "https://github.com/japanese-law-analysis/ocr_precedent.git"
//! ```
//!
//! # How to use
//!
//! ## 基本的な使い方
//!
//! ```sh
//! ocr_precedent --input "input.json"
//! ```
//!
//! で起動します。与えるJSONファイルは[listup_precedent](https://github.com/japanese-law-analysis/listup_precedent)で生成されるものです。
//!
//! 起動するとその場にtmpフォルダが作られ、そこに各PDFファイルなどがダウンロード・生成されます。
//!
//! そして`ocr_precedent`を起動したディレクトリに各判例テキストファイルが生成されます。
//!
//! ファイル名は`{事件番号}_{year}_{month}_{day}.txt`形式です。年月日は判決日です。
//!
//! ## オプション
//!
//! - `--tmp`：一時フォルダのフォルダ名を変更することができる
//! - `--do-not-use-cache`：PDFファイルがtmpフォルダにすでに存在している場合でも再度ダウンロードを実行ようにする
//! - `--force-re-ocr`：すでに生成済みテキストファイルが存在している場合でも再度OCR処理を実行する
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
  let file_path_err = format!("{file_name}_err.txt");
  let mut err_output = File::create(file_path_err).await?;
  if is_downloads {
    println!("[START] downloads: {url}");
    download_pdf(&file_path_pdf, url).await?;
    println!("[END] downloads: {url}");
  } else {
    println!("[Hit PDF Cache] {file_path_pdf}");
  };
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
      err_output.write_all(err_msg.as_bytes()).await?;
    }
    let err_msg_opt = ocr_img(&format!("{file_name}-{page_num}")).await;
    if let Some(err_msg) = err_msg_opt {
      err_output.write_all(err_msg.as_bytes()).await?;
    }
  }
  let txt_path_lst = (1..=pdf_size)
    .map(|i| format!("{file_name}-{i}.txt"))
    .collect::<Vec<_>>();
  join_ocr_file(&txt_path_lst, &file_path_txt).await?;
  err_output.flush().await?;
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
  /// 生成後のテキストファイルがあったとしても再度OCRしなおすかのフラグ
  #[arg(long, default_value_t = false)]
  force_re_ocr: bool,
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
    let cache_file_path = format!("{tmp_name}/{name}.pdf");
    let cache_path = Path::new(&cache_file_path);
    let txt_file_path = format!("{name}.txt");
    let txt_path = Path::new(&txt_file_path);
    let is_downloads = if !args.do_not_use_cache {
      // キャッシュを使うので、ファイルが無かったらダウンロードする
      !cache_path.exists()
    } else {
      // キャッシュを使わないので常にダウンロード
      true
    };
    let is_run = if !args.force_re_ocr {
      // 生成テキストファイルがなければ実行する
      !txt_path.exists()
    } else {
      // 常に実行
      true
    };
    if is_run {
      let url = v
        .get("full_pdf_link")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("full_pdf_linkフィールドが無い"))?;
      println!("[START] write: {name}");
      download_and_ocr(&name, url, tmp_name, is_downloads).await?;
      println!("[END] write: {name}");
    } else {
      println!("[Hit Text Cache] {name}({cache_file_path})");
    }
  }
  Ok(())
}
