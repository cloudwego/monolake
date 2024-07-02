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

# http proxy for traefik
./wrk -d 1m -c 10 -t 2 -R 2000 http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server2
./wrk -d 1m -c 10 -t 2 -R 2000 http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server3
./wrk -d 1m -c 10 -t 2 -R 2000 http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server4
./wrk -d 1m -c 10 -t 2 -R 2000 http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server5
