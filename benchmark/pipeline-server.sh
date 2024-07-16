export client_url=ec2-52-15-84-38.us-east-2.compute.amazonaws.com
export proxy_url=ec2-3-145-174-117.us-east-2.compute.amazonaws.com
export server_url=ec2-18-117-161-226.us-east-2.compute.amazonaws.com

# start server
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${server_url} -t 'sudo service nginx restart; sleep 3; sudo rm -f nginx-performance.csv; sudo ~/monolake/benchmark/performance-collect.sh nginx; echo "Please type exit to continue"; bash -l'
