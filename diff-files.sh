#!/bin/bash
git diff --no-index --word-diff=color --word-diff-regex=. $1 $2
