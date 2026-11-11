#!/usr/bin/env bash
# Temporary probe for the AI PR Agent. Delete this file once the review check has run.

message=$1

if [ $message == "ok" ]; then
  echo Hello $message
else
  echo not ok
fi
