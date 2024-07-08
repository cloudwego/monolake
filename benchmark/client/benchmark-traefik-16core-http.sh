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
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server2 > http-result-16c-traefik-tiny.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server3 > http-result-16c-traefik-small.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server4 > http-result-16c-traefik-medium.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8300/server5 > http-result-16c-traefik-large.txt
