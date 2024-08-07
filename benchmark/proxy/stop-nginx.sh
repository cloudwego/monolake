# stop nginx proxy service
sudo kill -15 $(ps aux | grep 'nginx' | awk '{print $2}')

sudo rm -f /var/log/nginx/error.log /var/log/nginx/access.log
