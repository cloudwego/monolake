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

# https proxy for envoy
./wrk -d 1m -c 640 -t 64 -R 150000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:5443/server2/ > https-result-4c-envoy-tiny.txt
./wrk -d 1m -c 640 -t 64 -R 140000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:5443/server3/ > https-result-4c-envoy-small.txt
./wrk -d 1m -c 640 -t 64 -R 80000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:5443/server4/ > https-result-4c-envoy-medium.txt
./wrk -d 1m -c 640 -t 64 -R 10000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:5443/server5/ > https-result-4c-envoy-large.txt
