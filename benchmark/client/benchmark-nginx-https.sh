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

# https proxy for nginx
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:8443/server2 > https-result-4c-nginx-tiny.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:8443/server3 > https-result-4c-nginx-small.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:8443/server4 > https-result-4c-nginx-medium.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency https://$MONOLAKE_BENCHMARK_PROXY_IP:8443/server5 > https-result-4c-nginx-large.txt
