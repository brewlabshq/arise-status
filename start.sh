#!/bin/bash
cd /home/sol/arise-status
exec cargo run /home/sol/arise-status/target/release/arise-status > /home/sol/arise-output.log
