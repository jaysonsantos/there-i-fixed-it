#!/bin/bash
set -ex
cd "$1"
mkdir destination.git
cd destination.git
git init --bare
# Older git does not have init --default-branch
git symbolic-ref HEAD refs/heads/main
cd ..
git clone destination.git setup
cd setup
if [ -z "$(git config --global user.email)" ]; then
    git config --global user.email test@test.com
fi
if [ -z "$(git config --global user.name)" ]; then
    git config --global user.name "Test User"
fi
echo "enabled = True" > file.py
git add .
git commit -m"Initial commit"
git branch -M main
git push -u origin main
