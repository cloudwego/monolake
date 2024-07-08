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
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8402 > http-result-16c-monolake-tiny.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8403 > http-result-16c-monolake-small.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8404 > http-result-16c-monolake-medium.txt
./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8405 > http-result-16c-monolake-large.txt

# http proxy for haproxy (not used)
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server2 > http-result-16c-haproxy-tiny.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server3 > http-result-16c-haproxy-small.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server4 > http-result-16c-haproxy-medium.txt
# ./wrk -d 1m -c 1000 -t 20 -R 2000 --latency http://$MONOLAKE_BENCHMARK_PROXY_IP:8200/server5 > http-result-16c-haproxy-large.txt
