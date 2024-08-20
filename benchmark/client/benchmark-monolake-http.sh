# run benchmark: make sure proxy and server all are running; run this script from client
if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

if [ -z "${MONOLAKE_BENCHMARK_PROXY_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_PROXY_IP=localhost
fi

if [ -z "${MONOLAKE_BENCHMARK_SERVER_IP+set}" ]; then
    export MONOLAKE_BENCHMARK_SERVER_IP=localhost
fi

cd $HOME/wrk2

# http proxy for monolake
./wrk -d 1m -c 640 -t 64 -R 200000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8402 > http-result-4c-monolake-tiny.txt
./wrk -d 1m -c 640 -t 64 -R 180000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8403 > http-result-4c-monolake-small.txt
./wrk -d 1m -c 640 -t 64 -R 100000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8404 > http-result-4c-monolake-medium.txt
./wrk -d 1m -c 640 -t 64 -R 100000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8405 > http-result-4c-monolake-large.txt

# ./wrk -d 1m -c 3500 -t 20 -R 80000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8402 > http-result-4c-monolake-tiny.txt
# ./wrk -d 1m -c 3500 -t 20 -R 73000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8403 > http-result-4c-monolake-small.txt
# ./wrk -d 1m -c 3500 -t 20 -R 70000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8404 > http-result-4c-monolake-medium.txt
# ./wrk -d 1m -c 120 -t 20 -R 7500 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8405 > http-result-4c-monolake-large.txt

# ./wrk -d 1m -c 1447 -t 20 -R 16000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8402 > http-result-4c-monolake-tiny.txt
# ./wrk -d 1m -c 1447 -t 20 -R 20000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8403 > http-result-4c-monolake-small.txt
# ./wrk -d 1m -c 1447 -t 20 -R 16000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8404 > http-result-4c-monolake-medium.txt
# ./wrk -d 1m -c 1200 -t 20 -R 4000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8405 > http-result-4c-monolake-large.txt

# http proxy for haproxy (not used)
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server2 > http-result-4c-haproxy-tiny.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server3 > http-result-4c-haproxy-small.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server4 > http-result-4c-haproxy-medium.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server5 > http-result-4c-haproxy-large.txt
