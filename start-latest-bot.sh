#!/bin/sh

bot_path='/root/geph-support-bot/';
config_path='/root/geph-support-bot/config.yaml';
# bot_path='/home/thisbefruit/GEPH4/geph-support-bot/';
# config_path='/home/thisbefruit/GEPH4/geph-support-bot/config.yaml';

cd $bot_path;
git pull;
RUST_LOG=geph_support_bot cargo run -- -c $config_path