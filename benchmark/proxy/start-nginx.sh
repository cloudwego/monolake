# start nginx proxy service
if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

sudo /usr/sbin/nginx -c $MONOLAKE_HOME/benchmark/proxy/nginx/nginx.conf -g "pid /var/run/nginx2.pid;" &
