#!/bin/bash
# KoadOS Telemetry Emitter (Phase 0)

KOAD_HOME="${KOAD_HOME:-$HOME/.koad-os}"
LOG_FILE="$KOAD_HOME/logs/telemetry.log"
EVENT=$1
AGENT=$2
SESSION_ID=$3

case $EVENT in
    "boot")
        printf "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [BOOT] agent=$AGENT session=$SESSION_ID
" >> $LOG_FILE
        ;;
    "shutdown")
        # Count turns from session history if available
        TURNS=0
        if [ -f "$HISTFILE" ]; then
            TURNS=$(wc -l < "$HISTFILE")
        fi
        printf "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [SHUTDOWN] agent=$AGENT session=$SESSION_ID turns=$TURNS
" >> $LOG_FILE
        ;;
    "metric")
        METRIC_NAME=$4
        METRIC_VALUE=$5
        printf "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [METRIC] agent=$AGENT session=$SESSION_ID $METRIC_NAME=$METRIC_VALUE
" >> $LOG_FILE
        ;;
    *)
        echo "Usage: $0 {boot|shutdown|metric} agent session_id [metric_name] [metric_value]"
        ;;
esac
