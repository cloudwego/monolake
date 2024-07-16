export client_url=ec2-52-15-84-38.us-east-2.compute.amazonaws.com
export proxy_url=ec2-3-145-174-117.us-east-2.compute.amazonaws.com
export server_url=ec2-18-117-161-226.us-east-2.compute.amazonaws.com

# start client
client_cmd='cd ~/monolake/benchmark/client; export MONOLAKE_BENCHMARK_PROXY_IP='
client_cmd+=$proxy_url
client_cmd+='; export MONOLAKE_BENCHMARK_SERVER_IP='
client_cmd+=$server_url
client_cmd+='; ./benchmark-nginx-http.sh; ./benchmark-nginx-https.sh; echo "Please type exit to continue"; bash -l'
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${client_url} -t $client_cmd
