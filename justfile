t := `cat /proc/cpuinfo | awk '/^processor/{print $3}' | tail -n 2 | head -n 1`
addr := 'http://127.0.0.1:1112/'
qkde := `curl --no-progress-meter http://127.0.0.1:1112/?q=kde | grep '<article ' | wc -l`
qwaf := `curl --no-progress-meter http://127.0.0.1:1112/?q=waffles | grep '<article ' | wc -l`

# Benchmark tts
bench:
    #!/bin/bash
    if   [ $(command -v wrk2) ] || [ $(command -v wrk ) ]; then
        work() {
            if   [ $(command -v wrk ) ]; then
                echo -e "\e[33mBenchmarking\e[39m::\e[32m$2\e[39m"
                wrk -c {{t}}k -t {{t}} -d 1m -L $1
            elif [ $(command -v wrk2) ]; then
                echo -e "\e[33mBenchmarking\e[39m::\e[32m$2\e[39m"
                wrk2 -c {{t}}k -t {{t}} -R 30k -d 1m $1 | tail -n 7
            fi
        }
        work {{addr}}           "Index"
        work {{addr}}?q=kde     "Query w/ {{qkde}} Results"
        work {{addr}}?q=waffles "Query w/ {{qwaf}} Results"
        work {{addr}}rando      "Redirect to Random Page"
    else
        echo "Please install wrk2 or wrk"
    fi
