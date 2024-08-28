# start envoy proxy service
if [ -z "${MONOLAKE_HOME+set}" ]; then
    export MONOLAKE_HOME=$HOME/monolake
fi

cd $MONOLAKE_HOME/benchmark/proxy/envoy
sudo $MONOLAKE_HOME/benchmark/proxy/envoy/envoy -c $MONOLAKE_HOME/benchmark/proxy/envoy/envoy.yaml &
