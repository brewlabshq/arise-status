#!/bin/bash
cd /home/linuxuser/arise-status
exec cargo run /home/sol/arise-status/target/release/arise-status > arise-output.log
