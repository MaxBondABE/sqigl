#!/bin/sh

cargo install just

wget "https://github.com/getzola/zola/releases/download/v0.19.2/zola-v0.19.2-x86_64-unknown-linux-gnu.tar.gz" -O zola.tar.gz
tar -xf zola.tar.gz
chmod ugo-w zola
mv zola /usr/local/bin
