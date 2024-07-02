# stop nginx proxy service
sudo kill -15 $(ps aux | grep 'nginx' | awk '{print $2}')
