# start traefik proxy service

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

cd $MONOLAKE_HOME/benchmark/proxy/traefik/
rm -f *log*

./traefik --configFile=traefik-static.toml &
