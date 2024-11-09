FILES="http-result-4c-monolake-tiny.txt http-result-4c-monolake-small.txt http-result-4c-monolake-medium.txt http-result-4c-monolake-large.txt http-result-4c-nginx-tiny.txt http-result-4c-nginx-small.txt http-result-4c-nginx-medium.txt http-result-4c-nginx-large.txt http-result-4c-traefik-tiny.txt http-result-4c-traefik-small.txt http-result-4c-traefik-medium.txt http-result-4c-traefik-large.txt http-result-4c-envoy-tiny.txt http-result-4c-envoy-small.txt http-result-4c-envoy-medium.txt http-result-4c-envoy-large.txt https-result-4c-monolake-tiny.txt https-result-4c-monolake-small.txt https-result-4c-monolake-medium.txt https-result-4c-monolake-large.txt https-result-4c-nginx-tiny.txt https-result-4c-nginx-small.txt https-result-4c-nginx-medium.txt https-result-4c-nginx-large.txt https-result-4c-traefik-tiny.txt https-result-4c-traefik-small.txt https-result-4c-traefik-medium.txt https-result-4c-traefik-large.txt https-result-4c-envoy-tiny.txt https-result-4c-envoy-small.txt https-result-4c-envoy-medium.txt https-result-4c-envoy-large.txt"
csv_filename="proxies-performance.csv"
# output_filename0="proxies-performance-boxes.png"
output_filename1="proxies-performance.png"
csv_filename2="proxies-performance-rotated.csv"
output_filename2="proxies-performance-rotated.png"
output_filename_tiny_throughput="tiny-throughput.png"
output_filename_small_throughput="small-throughput.png"
output_filename_medium_throughput="medium-throughput.png"
output_filename_large_throughput="large-throughput.png"
output_filename_tiny_qps="tiny-qps.png"
output_filename_small_qps="small-qps.png"
output_filename_medium_qps="medium-qps.png"
output_filename_large_qps="large-qps.png"
# output_filename3="proxies-performance-rotated-boxes.png"

echo "Case,Requests/sec,Transfer/sec,Server Error,Timeout" > $csv_filename

for f in $FILES
do
    echo "Processing $f file..."
    Line=$( tail -n 1 $f )
    Transfer=${Line##* }
    if [[ $Transfer == *"MB" ]]; then
        Bytes=${Transfer:0:${#Transfer} - 2}
        Bytes=$(echo "$Bytes * 1024 * 1024" | bc)
        # Bytes=$(echo "$Bytes * 100" | bc)
    elif [[ $Transfer == *"GB" ]]; then
        Bytes=${Transfer:0:${#Transfer} - 2}
        Bytes=$(echo "$Bytes * 1024 * 1024 *1024" | bc)
        # Bytes=$(echo "$Bytes * 102400" | bc)
    else
        Bytes=${Transfer:0:${#Transfer} - 2}
        Bytes=$(echo "$Bytes * 1024" | bc)
        # Bytes=$(echo "$Bytes / 10.24" | bc)
    fi
    Line=$( tail -n 2 $f | head -n 1 )
    Request=${Line##* }
    Line=$( tail -n 3 $f | head -n 1 )
    if [[ $Line == *"Non-2xx"* ]]; then
        ServerError=${Line##* }
        Line=$( tail -n 4 $f | head -n 1 )
    else
        ServerError="0"
    fi
    if [[ $Line == *"Socket errors"* ]]; then
        Timeout=${Line##* }
    else
        Timeout="0"
    fi
    Case=`echo "$f" | cut -d'.' -f1`
    echo "$Case,$Request,$Bytes,$ServerError,$Timeout" >> $csv_filename
done

python3 performance-csv-convert.py

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
    # Proxy Services Performance
    # set output "$output_filename1"
    # set title "Proxy Services Performance"
    # plot "$csv_filename" using 2:xticlabels(1) with lines smooth unique lw 2 lt rgb "#4848d6" t "Requests/sec",\
    #      "$csv_filename" using 3:xticlabels(1) with lines smooth unique lw 2 lt rgb "#b40000" t "Transfer 10KB/sec", \
    #      "$csv_filename" using 4:xticlabels(1) with lines smooth unique lw 2 lt rgb "#ed8004" t "Server Error", \
    #      "$csv_filename" using 5:xticlabels(1) with lines smooth unique lw 2 lt rgb "#48d65b" t "Timeout",

    # set output "$output_filename0"
    set output "$output_filename1"
    set yrange [1000:*]
    set logscale y
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 1

    plot "$csv_filename" u 3:xtic(1) ti col,\
        '' u 2 ti col,
        # '' u 4 ti col,\
        # '' u 5 ti col,
EOF

echo "Plotting graphs rotated..."
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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

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
    # Proxy Services Performance
    # set output "$output_filename2"
    # set title "Proxy Services Performance By Payload"

    # plot "$csv_filename2" using 2:xticlabels(1) with lines smooth unique lw 2 lt rgb "#ff0000" t "Tiny Requests/sec",\
    #      "$csv_filename2" using 3:xticlabels(1) with lines smooth unique lw 2 lt rgb "#00ff00" t "Small Requests/sec", \
    #      "$csv_filename2" using 4:xticlabels(1) with lines smooth unique lw 2 lt rgb "#0000ff" t "Medium Requests/sec", \
    #      "$csv_filename2" using 5:xticlabels(1) with lines smooth unique lw 2 lt rgb "#000000" t "Large Requests/sec", \
    #      "$csv_filename2" using 6:xticlabels(1) with lines smooth unique lw 2 lt rgb "#800000" t "Tiny Transfer 10KB/sec",\
    #      "$csv_filename2" using 7:xticlabels(1) with lines smooth unique lw 2 lt rgb "#008000" t "Small Transfer 10KB/sec", \
    #      "$csv_filename2" using 8:xticlabels(1) with lines smooth unique lw 2 lt rgb "#000080" t "Medium Transfer 10KB/sec", \
    #      "$csv_filename2" using 9:xticlabels(1) with lines smooth unique lw 2 lt rgb "#808080" t "Large Transfer 10KB/sec",

    # set output "$output_filename3"
    set output "$output_filename2"
    set title "Proxy Services Performance By Proxy"

    set yrange [1000:*]
    set logscale y
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" u 6:xtic(1) ti col,\
        '' u 7 ti col,\
        '' u 8 ti col,\
        '' u 9 ti col,\
        '' u 2 ti col,\
        '' u 3 ti col,\
        '' u 4 ti col,\
        '' u 5 ti col,
EOF

echo "Plotting itemized graphs rotated..."
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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

    # Set separator to comma
    set datafile separator ","

    # Move legend to the bottom
    set key bmargin center box lt rgb "#d8d8d8" horizontal

    set output "$output_filename_tiny_throughput"
    set title "Proxy Throughput of HTTP/S Tiny Payload"

    # set yrange [1000:*]
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" using 6:xtic(1) ti '' with boxes fc rgb "#4848d6"
EOF

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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

    # Set separator to comma
    set datafile separator ","

    # Move legend to the bottom
    set key bmargin center box lt rgb "#d8d8d8" horizontal

    set output "$output_filename_small_throughput"
    set title "Proxy Throughput of HTTP/S Small Payload"

    # set yrange [1000:*]
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" using 7:xtic(1) ti '' with boxes fc rgb "#b40000"
EOF

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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

    # Set separator to comma
    set datafile separator ","

    # Move legend to the bottom
    set key bmargin center box lt rgb "#d8d8d8" horizontal

    set output "$output_filename_medium_throughput"
    set title "Proxy Throughput of HTTP/S Medium Payload"

    # set yrange [1000:*]
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" using 8:xtic(1) ti '' with boxes fc rgb "#ed8004"
EOF

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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

    # Set separator to comma
    set datafile separator ","

    # Move legend to the bottom
    set key bmargin center box lt rgb "#d8d8d8" horizontal

    set output "$output_filename_large_throughput"
    set title "Proxy Throughput of HTTP/S Large Payload"

    # set yrange [1000:*]
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" using 9:xtic(1) ti '' with boxes fc rgb "#48d65b"
EOF

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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

    # Set separator to comma
    set datafile separator ","

    # Move legend to the bottom
    set key bmargin center box lt rgb "#d8d8d8" horizontal

    set output "$output_filename_tiny_qps"
    set title "Proxy QPS of HTTP/S Tiny Payload"

    # set yrange [1000:*]
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" using 2:xtic(1) ti '' with boxes fc rgb "#4848d6"
EOF

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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

    # Set separator to comma
    set datafile separator ","

    # Move legend to the bottom
    set key bmargin center box lt rgb "#d8d8d8" horizontal

    set output "$output_filename_small_qps"
    set title "Proxy QPS of HTTP/S Small Payload"

    # set yrange [1000:*]
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" using 3:xtic(1) ti '' with boxes fc rgb "#b40000"
EOF

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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

    # Set separator to comma
    set datafile separator ","

    # Move legend to the bottom
    set key bmargin center box lt rgb "#d8d8d8" horizontal

    set output "$output_filename_medium_qps"
    set title "Proxy QPS of HTTP/S Medium Payload"

    # set yrange [1000:*]
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" using 4:xtic(1) ti '' with boxes fc rgb "#ed8004"
EOF

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
    # set xtics rotate

    # Set graph size relative to the canvas
    set size 1,0.85

    set boxwidth 0.5
    set style fill solid

    # Set separator to comma
    set datafile separator ","

    # Move legend to the bottom
    set key bmargin center box lt rgb "#d8d8d8" horizontal

    set output "$output_filename_large_qps"
    set title "Proxy QPS of HTTP/S Large Payload"

    # set yrange [1000:*]
    set ytics format "%.1s%c"

    set style data histogram
    set style histogram cluster gap 1
    set style fill solid border -1
    set boxwidth 0.9

    plot "$csv_filename2" using 5:xtic(1) ti '' with boxes fc rgb "#48d65b"
EOF

echo "copy generated images to ../images/README"
cp performance-metrices-monolake.png ../images/README/
# cp proxies-performance-rotated.png ../images/README/
# cp proxies-performance.png ../images/README/
cp nginx-http-latency.png ../images/README/
cp all-http-latency.png ../images/README/
cp all-latency-https.png ../images/README/
# cp large-qps.png ../images/README/
# cp medium-qps.png ../images/README/
# cp small-qps.png ../images/README/
# cp tiny-qps.png ../images/README/
# cp large-throughput.png ../images/README/
# cp medium-throughput.png ../images/README/
# cp small-throughput.png ../images/README/
# cp tiny-throughput.png ../images/README/
cp all-latency-https-large.png ../images/README/
cp all-latency-https-medium.png ../images/README/
cp all-latency-https-small.png ../images/README/
cp all-latency-https-tiny.png ../images/README/
cp all-http-large-latency.png ../images/README/
cp all-http-medium-latency.png ../images/README/
cp all-http-small-latency.png ../images/README/
cp all-http-tiny-latency.png ../images/README/
cp $output_filename1 ../images/README/
cp $output_filename2 ../images/README/
cp $output_filename_tiny_throughput ../images/README/
cp $output_filename_small_throughput ../images/README/
cp $output_filename_medium_throughput ../images/README/
cp $output_filename_large_throughput ../images/README/
cp $output_filename_tiny_qps ../images/README/
cp $output_filename_small_qps ../images/README/
cp $output_filename_medium_qps ../images/README/
cp $output_filename_large_qps ../images/README/
