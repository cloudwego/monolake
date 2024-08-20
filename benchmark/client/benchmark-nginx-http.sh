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

# http proxy for nginx
./wrk -d 1m -c 640 -t 64 -R 210000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server2 > http-result-4c-nginx-tiny.txt
./wrk -d 1m -c 640 -t 64 -R 200000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server3 > http-result-4c-nginx-small.txt
./wrk -d 1m -c 640 -t 64 -R 120000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server4 > http-result-4c-nginx-medium.txt
./wrk -d 1m -c 640 -t 64 -R 10000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server5 > http-result-4c-nginx-large.txt

# ./wrk -d 1m -c 3500 -t 20 -R 31300 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server2 > http-result-4c-nginx-tiny.txt
# ./wrk -d 1m -c 3500 -t 20 -R 30000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server3 > http-result-4c-nginx-small.txt
# ./wrk -d 1m -c 3500 -t 20 -R 28800 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server4 > http-result-4c-nginx-medium.txt
# ./wrk -d 1m -c 1200 -t 20 -R 7500 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server5 > http-result-4c-nginx-large.txt

# ./wrk -d 1m -c 1447 -t 20 -R 16000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server2 > http-result-4c-nginx-tiny.txt
# ./wrk -d 1m -c 1447 -t 20 -R 20000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server3 > http-result-4c-nginx-small.txt
# ./wrk -d 1m -c 1447 -t 20 -R 16000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server4 > http-result-4c-nginx-medium.txt
# ./wrk -d 1m -c 1200 -t 20 -R 4000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8100/server5 > http-result-4c-nginx-large.txt
