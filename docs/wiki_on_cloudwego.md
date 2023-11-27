# Overview

The cloudwego folder contains all the wiki content host on [CloudWeGo](https://www.cloudwego.io).

## Preview Wiki

``` bash
# install hugo
brew install hugo

# clone the cloudwego repo
git clone https://github.com/cloudwego/cloudwego.github.io

# sync the content to cloudwego folder
rsync -a cloudwego cloudwego.github.io/content/en/docs/monolake

# run hugo server
cd cloudwego.github.io
hugo server --bind ::1 -D
```
