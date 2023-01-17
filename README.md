[![Workflow Status](https://github.com/japanese-law-analysis/pdf2txt_precedent/workflows/Rust%20CI/badge.svg)](https://github.com/japanese-law-analysis/pdf2txt_precedent/actions?query=workflow%3A%22Rust%2BCI%22)

# pdf2txt_precedent

[listup_precedent](https://github.com/japanese-law-analysis/listup_precedent)で作成した裁判例の一覧をもとに、[裁判所のHP](https://www.courts.go.jp)から判決文PDFファイルをダウンロードしてテキストに直すソフトウェアです。

## Install
requires:
- [tesseract](https://github.com/tesseract-ocr/tesseract)
- tesseract-ocr-jpn
- ImageMagick
- poppler-utils

ubuntu:
```sh
sudo apt update
sudo apt install tesseract-ocr libtesseract-dev tesseract-ocr-jpn imagemagick poppler-utils
cargo install --git "https://github.com/japanese-law-analysis/pdf2txt_precedent.git"
```

## How to use

### 基本的な使い方

```sh
pdf2txt_precedent --input "input.json"
```

で起動します。与えるJSONファイルは[listup_precedent](https://github.com/japanese-law-analysis/listup_precedent)で生成されるものです。

起動するとその場にtmpフォルダが作られ、そこに各PDFファイルなどがダウンロード・生成されます。

そして`pdf2txt_precedent`を起動したディレクトリに各判例テキストファイルが生成されます。

ファイル名は`{事件番号}_{year}_{month}_{day}.txt`形式です。年月日は判決日です。

### オプション

- `--tmp`：一時フォルダのフォルダ名を変更することができる
- `--output`：生成ファイルを出力するフォルダを変更することができる
- `--mode`：テキスト抽出に用いる技術を選ぶことができる
  - `p2t`：`pdftotext`コマンドを使用した抽出を行う
  - `ocr`：OCRを用いた抽出を行う
- `--do-not-use-cache`：PDFファイルがtmpフォルダにすでに存在している場合でも再度ダウンロードを実行ようにする
- `--force-re-run`：すでに生成済みテキストファイルが存在している場合でも再度処理を実行する

---
[MIT License](https://github.com/japanese-law-analysis/pdf2txt_precedent/blob/master/LICENSE)
(c) 2023 Naoki Kaneko (a.k.a. "puripuri2100")


License: MIT
