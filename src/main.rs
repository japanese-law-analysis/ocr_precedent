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
//! cargo install --git "https://github.com/japanese-law-analysis/pdf2txt_precedent.git"
//! ```
//!
//! # How to use
//!
//! ## 基本的な使い方
//!
//! ```sh
//! pdf2txt_precedent --input "input.json"
//! ```
//!
//! で起動します。与えるJSONファイルは[listup_precedent](https://github.com/japanese-law-analysis/listup_precedent)で生成されるものです。
//!
//! 起動するとその場にtmpフォルダが作られ、そこに各PDFファイルなどがダウンロード・生成されます。
//!
//! そして`pdf2txt_precedent`を起動したディレクトリに各判例テキストファイルが生成されます。
//!
//! ファイル名は`{事件番号}_{year}_{month}_{day}_{裁判の種類}.txt`形式です。年月日は判決日です。
//!
//! ## オプション
//!
//! - `--tmp`：一時フォルダのフォルダ名を変更することができる
//! - `--output`：生成ファイルを出力するフォルダを変更することができる
//! - `--mode`：テキスト抽出に用いる技術を選ぶことができる
//!   - `p2t`：`pdftotext`コマンドを使用した抽出を行う
//!   - `ocr`：OCRを用いた抽出を行う
//! - `--do-not-use-cache`：PDFファイルがtmpフォルダにすでに存在している場合でも再度ダウンロードを実行ようにする
//! - `--force-re-run`：すでに生成済みテキストファイルが存在している場合でも再度処理を実行する
//!
//! ---
//! [MIT License](https://github.com/japanese-law-analysis/pdf2txt_precedent/blob/master/LICENSE)
//! (c) 2023 Naoki Kaneko (a.k.a. "puripuri2100")
//!

use anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use regex::Regex;
use serde_json::{Map, Value};
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

async fn pdf2txt_img(name: &str) -> Option<String> {
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

async fn join_pdf2txt_text(text: &str) -> String {
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

async fn join_pdf2txt_file(file_path_lst: &[String], output_path: &str) -> Result<()> {
  let mut s = String::new();
  let mut stream = tokio_stream::iter(file_path_lst);
  while let Some(file_path) = stream.next().await {
    let file_contents = fs::read_to_string(file_path).await?;
    s.push_str(file_contents.trim());
  }
  let s = join_pdf2txt_text(&s).await;
  let mut output = File::create(output_path).await?;
  output.write_all(s.as_bytes()).await?;
  output.flush().await?;
  Ok(())
}

async fn download_and_pdftotext(
  name: &str,
  url: &str,
  tmp_name: &str,
  output_name: &str,
  is_downloads: bool,
) -> Result<()> {
  let file_name = format!("{tmp_name}/{name}");
  let file_path_pdf = format!("{file_name}.pdf");
  let file_path_generate_txt = format!("{file_name}.txt");
  let file_path_txt = format!("{output_name}/{name}.txt");
  let file_path_err = format!("{file_name}_err.txt");
  let mut txt_output = File::create(file_path_txt).await?;
  let mut err_txt = String::new();
  if is_downloads {
    println!("[START] downloads: {url}");
    download_pdf(&file_path_pdf, url).await?;
    println!("[END] downloads: {url}");
  } else {
    println!("[Hit PDF Cache] {file_path_pdf}");
  };
  let output = Command::new("pdftotext")
    .arg(file_path_pdf)
    .arg("-raw")
    .output()
    .await
    .ok();
  if let Some(output) = output {
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !stderr.is_empty() {
      err_txt.push_str(&stderr);
      err_txt.push('\n');
    };
  }
  if let Ok(generate_txt) = fs::read_to_string(&file_path_generate_txt).await {
    let mut line_stream = tokio_stream::iter(generate_txt.lines());
    while let Some(line) = line_stream.next().await {
      let is_page_or_line_num_re = Regex::new(r"^(\s*-?\s*\d+\s*-?\s*)|(\s+)$").unwrap();
      if !is_page_or_line_num_re.is_match(line) {
        txt_output.write_all(line.as_bytes()).await?;
        txt_output.write_all(b"\n").await?;
      }
    }
  } else {
    err_txt.push_str(&format!(
      "'{}': No such file or directory\n",
      &file_path_generate_txt
    ));
  }
  txt_output.flush().await?;
  if !err_txt.is_empty() {
    let mut err_output = File::create(file_path_err).await?;
    err_output.write_all(err_txt.as_bytes()).await?;
    err_output.flush().await?;
  }
  Ok(())
}

async fn download_and_ocr(
  name: &str,
  url: &str,
  tmp_name: &str,
  output_name: &str,
  is_downloads: bool,
) -> Result<()> {
  let file_name = format!("{tmp_name}/{name}");
  let file_path_pdf = format!("{file_name}.pdf");
  let file_path_txt = format!("{output_name}/{name}.txt");
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
    let err_msg_opt = pdf2txt_img(&format!("{file_name}-{page_num}")).await;
    if let Some(err_msg) = err_msg_opt {
      err_output.write_all(err_msg.as_bytes()).await?;
    }
  }
  let txt_path_lst = (1..=pdf_size)
    .map(|i| format!("{file_name}-{i}.txt"))
    .collect::<Vec<_>>();
  join_pdf2txt_file(&txt_path_lst, &file_path_txt).await?;
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
  /// 生成ファイルを出力するフォルダ
  #[arg(short, long, default_value_t=String::from("."))]
  output: String,
  /// PDFのキャッシュを作成しない場合に付けるフラグ
  #[arg(long, default_value_t = false)]
  do_not_use_cache: bool,
  /// 生成後のテキストファイルがあったとしても再度実行しなおすかのフラグ
  #[arg(long, default_value_t = false)]
  force_re_run: bool,
  /// 生テキスト抽出をどの方法で行うかの選択
  #[arg(short, long, value_enum, default_value_t=Mode::P2T)]
  mode: Mode,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, ValueEnum)]
enum Mode {
  /// `pdftotext`コマンドを使用する
  P2T,
  /// OCRを使用する
  OCR,
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = Args::parse();
  let tmp_name = &args.tmp;
  let output_name = &args.output;
  fs::create_dir_all(tmp_name).await?;
  fs::create_dir_all(output_name).await?;
  let input_file_path = &args.input;
  let input_json = fs::read_to_string(input_file_path).await?;
  let input_json_lst: Map<String, Value> = serde_json::from_str(&input_json)?;
  let mut json_stream = tokio_stream::iter(input_json_lst);
  while let Some((name, v)) = json_stream.next().await {
    let case_number = v
      .get("case_number")
      .and_then(|v| v.as_str())
      .ok_or_else(|| anyhow!("case_numberフィールドが無い"))?;
    println!("case_number: {case_number}");
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
    let is_run = if !args.force_re_run {
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
      match &args.mode {
        Mode::P2T => {
          download_and_pdftotext(&name, url, tmp_name, output_name, is_downloads).await?
        }
        Mode::OCR => download_and_ocr(&name, url, tmp_name, output_name, is_downloads).await?,
      };
      println!("[END] write: {name}");
    } else {
      println!("[Hit Text Cache] {name}({cache_file_path})");
    }
  }
  Ok(())
}
