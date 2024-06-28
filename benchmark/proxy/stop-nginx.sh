# stop nginx proxy service
kill -15 $(ps aux | grep 'nginx' | awk '{print $2}')
