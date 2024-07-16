export client_url=ec2-52-15-84-38.us-east-2.compute.amazonaws.com
export proxy_url=ec2-3-145-174-117.us-east-2.compute.amazonaws.com
export server_url=ec2-18-117-161-226.us-east-2.compute.amazonaws.com

#manual update proxy configurations
#ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t 'cd ~/monolake/benchmark/proxy; MONOLAKE_BENCHMARK_SERVER_IP=${server_url} ./update-server-ip.sh; bash -l'

#then start proxy
proxy_cmd='export MONOLAKE_BENCHMARK_PROXY_IP='
proxy_cmd+=$proxy_url
proxy_cmd+='; export MONOLAKE_BENCHMARK_SERVER_IP='
proxy_cmd+=$server_url
proxy_cmd+='; ~/monolake/benchmark/proxy/start-monolake.sh; sleep 3; rm -f ~/monolake-performance.csv; ~/monolake/benchmark/performance-collect.sh monolake; echo "Please type exit to continue"; bash -l'
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${proxy_url} -t $proxy_cmd
