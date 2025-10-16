#!/bin/bash
dot -Tsvg $1 -o $2.svg
open $2.svg
