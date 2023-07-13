#!/bin/bash

# Function to print progress logs
function log_progress {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $1"
}

# Clone cloudwego.github.io repository
log_progress "Cloning cloudwego.github.io repository..."
git clone https://github.com/cloudwego/cloudwego.github.io

# Copy contents of docs folder to cloudwego.github.io/content/en/docs/monolake/
log_progress "Copying contents to cloudwego.github.io..."
cp -R docs cloudwego.github.io/content/en/docs/monolake

# Start Hugo server for preview
log_progress "Starting Hugo server for preview..."
cd cloudwego.github.io
hugo server -D
