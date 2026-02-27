#!/bin/bash

# Define the log file (all writes go here)
LOG_FILE="/Users/bhavyabhatt/Desktop/bhavya/projects/contract-sdk/python/.claude/write-log.txt"

# Append to log file with timestamp and file info
cat >> "$LOG_FILE"

# Allow the write to proceed
exit 0