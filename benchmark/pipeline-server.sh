export client_url=ec2-18-116-241-44.us-east-2.compute.amazonaws.com
export proxy_url=ec2-18-226-87-157.us-east-2.compute.amazonaws.com
export server_url=ec2-3-133-91-193.us-east-2.compute.amazonaws.com

# start server
ssh -i $HOME/ssh/monolake-benchmark.pem ec2-user@${server_url} -t 'sudo service nginx restart; sleep 3; sudo rm -f nginx-performance.csv; sudo ~/monolake/benchmark/performance-collect.sh nginx; echo "Please type exit to continue"; bash -l'
