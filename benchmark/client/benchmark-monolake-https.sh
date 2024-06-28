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

# https proxy for monolake
./wrk 'https://$MONOLAKE_BENCHMARK_PROXY_IP:6442/' -d 1m -c 10 -t 2; ./wrk 'https://$MONOLAKE_BENCHMARK_PROXY_IP:6443/' -d 1m -c 10 -t 2; ./wrk 'https://$MONOLAKE_BENCHMARK_PROXY_IP:6444/' -d 1m -c 10 -t 2; ./wrk 'https://$MONOLAKE_BENCHMARK_PROXY_IP:6445/' -d 1m -c 10 -t 2

# https proxy for haproxy (not used)
#./wrk 'https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server2' -d 1m -c 10 -t 2; ./wrk 'https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server3' -d 1m -c 10 -t 2; ./wrk 'https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server4' -d 1m -c 10 -t 2; ./wrk 'https://$MONOLAKE_BENCHMARK_PROXY_IP:9443/server5' -d 1m -c 10 -t 2
