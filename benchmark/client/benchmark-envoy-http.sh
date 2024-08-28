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

# http proxy for envoy
./wrk -d 1m -c 640 -t 64 -R 210000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8500/server2/ > http-result-4c-envoy-tiny.txt
./wrk -d 1m -c 640 -t 64 -R 200000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8500/server3/ > http-result-4c-envoy-small.txt
./wrk -d 1m -c 640 -t 64 -R 120000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8500/server4/ > http-result-4c-envoy-medium.txt
./wrk -d 1m -c 640 -t 64 -R 10000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8500/server5/ > http-result-4c-envoy-large.txt
