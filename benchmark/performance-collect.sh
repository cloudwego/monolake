#!/bin/bash

# process id to monitor
p_name=$1
# pid=`/bin/ps -fu $USER| grep "$p_name" | grep -v "grep" | awk '{print $2}'`
pid=`pidof $p_name`
echo "pid=$pid"

if [ -z $1 ]; then
  echo "ERROR: Process not specified."
  echo
  echo "Usage: $(basename "$0") <Process Name>"
  exit 1
fi

# check if process exists
kill -0 $pid > /dev/null 2>&1
pid_exist=$?

if [ $pid_exist != 0 ]; then
  echo "ERROR: Process ID $pid not found."
  exit 1
fi

current_time=$(date +"%Y_%m_%d_%H%M")
dir_name="."
csv_filename="${1}-performance.csv"


echo "Writing data to CSV file $csv_filename..."
rm -f $csv_filename
touch $csv_filename

# write CSV headers
echo "Time,CPU,Memory,TCP Connections,Thread Count" >> $csv_filename

# check if process exists
kill -0 $pid > /dev/null 2>&1
pid_exist=$?

# collect until process exits
while [ $pid_exist == 0 ]; do
  # check if process exists
  kill -0 $pid > /dev/null 2>&1
  pid_exist=$?

  if [ $pid_exist == 0 ]; then
    # read cpu and mem percentages
    timestamp=$(date +"%b %d %H:%M:%S")
    cpu_mem_usage=$(top -b -n 1 | grep -w -E "^ *$pid" | awk '{print $9 "," $10}')
    tcp_cons=$(lsof -i -a -p $pid -w | tail -n +2 | wc -l)
    tcount=$(ps -o nlwp h $pid | tr -d ' ')

    # write CSV row
    echo "$timestamp,$cpu_mem_usage,$tcp_cons,$tcount" >> $csv_filename
    sleep 1
  fi
done
