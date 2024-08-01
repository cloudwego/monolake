export client_url=3.133.229.116
export proxy_url=3.19.41.190
export server_url=3.22.140.218
export proxy_private_url=172.31.7.16
export server_private_url=172.31.22.170

#manual update proxy configurations
#ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; MONOLAKE_BENCHMARK_SERVER_IP=${server_url} ./update-server-ip.sh; bash -l'

#then start proxy
proxy_cmd='export MONOLAKE_BENCHMARK_PROXY_IP='
proxy_cmd+=$proxy_private_url
proxy_cmd+='; export MONOLAKE_BENCHMARK_SERVER_IP='
proxy_cmd+=$server_private_url
proxy_cmd+='; ~/monolake/benchmark/proxy/start-traefik.sh; sleep 3; rm -f ~/traefik-performance.csv; ~/monolake/benchmark/performance-collect.sh traefik; echo "Please type exit to continue"; bash -l'
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t $proxy_cmd
