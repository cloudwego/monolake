#!/bin/bash

# process to monitor
process_name=$1
dir_name="."
csv_filename="${1}-performance.csv"

# Read collected metrices from the CSV file and plot graphs
#
# This function will end script execution.
#
# This function is to be called after an interrupt like SIGINT or SIGKILL
# is received.
#
function plotGraph() {

  # bring cursor to next line after interrupt
  echo

  # plot graphs if there is a data file
  if [ -f $csv_filename ]; then
    echo "Plotting graphs..."
    gnuplot <<- EOF
      # Output to png with a font size of 10, using pngcairo for anti-aliasing
      set term pngcairo size 1024,800 noenhanced font "Helvetica,10"

      # Set border color around the graph
      set border ls 50 lt rgb "#939393"

      # Hide left and right vertical borders
      set border 16 lw 0
      set border 64 lw 0

      # Set tic color
      set tics nomirror textcolor rgb "#939393"

      # Set horizontal lines on the ytics
      set grid ytics lt 1 lc rgb "#d8d8d8" lw 2

      # Rotate x axis lables
      set xtics rotate

      # Set graph size relative to the canvas
      set size 1,0.85

      # Set separator to comma
      set datafile separator ","

      # Move legend to the bottom
      set key bmargin center box lt rgb "#d8d8d8" horizontal

      # Plot graph,
      # xticlabels(1) - first column as x tic labels
      # "with lines" - line graph
      # "smooth unique"
      # "lw 2" - line width
      # "lt rgb " - line style color
      # "t " - legend labels
      #
      # CPU and memory usage
      set output "${dir_name}/cpu-mem-usage-${process_name}.png"
      set title "CPU and Memory Usage for Proces ${process_name}"
      plot "$csv_filename" using 2:xticlabels(1) with lines smooth unique lw 2 lt rgb "#4848d6" t "CPU Usage %",\
       "$csv_filename" using 3:xticlabels(1) with lines smooth unique lw 2 lt rgb "#b40000" t "Memory Usage %"

      # TCP count
      set output "${dir_name}/tcp-count-${process_name}.png"
      set title "TCP Connections Count for Proces ${process_name}"
      plot "$csv_filename" using 4:xticlabels(1) with lines smooth unique lw 2 lt rgb "#ed8004" t "TCP Connection Count"

      # Thread count
      set output "${dir_name}/thread-count-${process_name}.png"
      set title "Thread Count for Proces ${process_name}"
      plot "$csv_filename" using 5:xticlabels(1) with lines smooth unique lw 2 lt rgb "#48d65b" t "Thread Count"

       # All together
       set output "${dir_name}/performance-metrices-${process_name}.png"
       set title "Performance Metrics for Proces ${process_name}"
       plot "$csv_filename" using 2:xticlabels(1) with lines smooth unique lw 2 lt rgb "#4848d6" t "CPU Usage %",\
        "$csv_filename" using 3:xticlabels(1) with lines smooth unique lw 2 lt rgb "#b40000" t "Memory Usage %", \
        "$csv_filename" using 4:xticlabels(1) with lines smooth unique lw 2 lt rgb "#ed8004" t "TCP Connection Count", \
        "$csv_filename" using 5:xticlabels(1) with lines smooth unique lw 2 lt rgb "#48d65b" t "Thread Count"
EOF
  fi

  echo "Done!"
  exit 0
}

# add SIGINT & SIGTERM trap
trap "plotGraph" SIGINT SIGTERM SIGKILL

# draw graph
plotGraph
