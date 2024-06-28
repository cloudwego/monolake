# start nginx proxy service
sudo /usr/sbin/nginx -c ~/monolake/benchmark/proxy/nginx/nginx.conf -g "pid /var/run/nginx2.pid;" &
