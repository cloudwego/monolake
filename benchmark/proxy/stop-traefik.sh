# stop traefik proxy service
if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

kill -15 $(ps aux | grep 'traefik' | awk '{print $2}')

cd $MONOLAKE_HOME/benchmark/proxy/traefik/
rm -f *log*
