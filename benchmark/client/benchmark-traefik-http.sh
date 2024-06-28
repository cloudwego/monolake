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

cd $MONOLAKE_HOME/benchmark/client/wrk2

# http proxy for traefik
./wrk 'http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server2' -d 1m -c 10 -t 2; ./wrk 'http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server3' -d 1m -c 10 -t 2; ./wrk 'http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server4' -d 1m -c 10 -t 2; ./wrk 'http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server5' -d 1m -c 10 -t 2
