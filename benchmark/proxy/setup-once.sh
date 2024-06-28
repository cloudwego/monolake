# start nginx server

if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

cd $MONOLAKE_HOME/benchmark/proxy
./monolake/setup-once.sh
./nginx/setup-once.sh
./traefik/setup-once.sh
