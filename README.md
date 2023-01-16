[![Workflow Status](https://github.com/japanese-law-analysis/ocr_precedent/workflows/Rust%20CI/badge.svg)](https://github.com/japanese-law-analysis/ocr_precedent/actions?query=workflow%3A%22Rust%2BCI%22)

# ocr_precedent

判例のPDFをOCRでテキストに変換するプログラム

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
cargo install --git "https://github.com/japanese-law-analysis/ocr_precedent.git"
```

## How to use

```sh
ocr_precedent --input "input.json"
```

で起動します。与えるJSONファイルは[listup_precedent](https://github.com/japanese-law-analysis/listup_precedent)で生成されるものです。

起動するとその場にtmpフォルダが作られ、そこに各PDFファイルなどがダウンロード・生成されます。

そして`ocr_precedent`を起動したディレクトリに各判例テキストファイルが生成されます。

ファイル名は`{事件番号}_{year}_{month}_{day}.txt`形式です。年月日は判決日です。

---
[MIT License](https://github.com/japanese-law-analysis/ocr_precedent/blob/master/LICENSE)
(c) 2021 Naoki Kaneko (a.k.a. "puripuri2100")


License: MIT
