#!/usr/bin/env bash
# Synthetic-only release gate. Never connects to configured application servers.
set -euo pipefail

if [[ ${GITHUB_ACTIONS:-} != true || ${RUNNER_ENVIRONMENT:-} != github-hosted ]]; then
  echo 'This launcher requires an ephemeral GitHub-hosted CI runner.' >&2
  exit 1
fi
[[ ${GITHUB_RUN_ID:-} =~ ^[0-9]+$ && ${GITHUB_RUN_ATTEMPT:-} =~ ^[0-9]+$ ]]
command -v mysql >/dev/null
command -v mysqldump >/dev/null

fixture_prefix="cb-multidb-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
fixture_label="calmbackup.ci.fixture=${fixture_prefix}"
fixture_containers=()

cleanup_fixtures() {
  local result=$? container owner
  trap - EXIT
  for container in "${fixture_containers[@]}"; do
    owner=$(docker inspect --format '{{index .Config.Labels "calmbackup.ci.fixture"}}' "$container") || continue
    if [[ $owner != "$fixture_prefix" ]]; then
      echo 'Fixture ownership changed; refusing cleanup.' >&2
      result=1
      continue
    fi
    if [[ $result != 0 ]]; then
      docker logs --tail 60 "$container" || true
    fi
    docker rm --force "$container" >/dev/null || result=1
  done
  exit "$result"
}
trap cleanup_fixtures EXIT

tests=(
  mysql_multi_database_snapshot_restores_original_schemas_and_values
  mysql_joint_snapshot_is_consistent_during_cross_schema_transactions
)
for index in "${!tests[@]}"; do
  current_pair=()
  for port in 3306 3307; do
    container="${fixture_prefix}-${index}-${port}"
    # docker create refuses name collisions; only successfully created fixtures
    # are added to the cleanup list. No existing database is dropped or reused.
    docker create --name "$container" --label "$fixture_label" \
      --publish "127.0.0.1:${port}:3306" --memory 1g --memory-swap 1g --cpus 1 \
      --pids-limit 256 --restart no --tmpfs /var/lib/mysql:rw,size=536870912 \
      -e MYSQL_ALLOW_EMPTY_PASSWORD=yes -e MYSQL_ROOT_HOST=% \
      mysql:8.0.45 --event-scheduler=OFF --local-infile=OFF --skip-log-bin \
      --innodb-buffer-pool-size=128M >/dev/null
    fixture_containers+=("$container")
    current_pair+=("$container")
    docker start "$container" >/dev/null
    healthy=false
    for attempt in {1..90}; do
      if docker exec "$container" mysql -uroot --batch --skip-column-names \
          -e 'SELECT @@server_uuid' >/dev/null 2>&1 \
          && mysql -h127.0.0.1 -P"$port" -uroot --batch --skip-column-names \
          -e 'SELECT @@server_uuid' >/dev/null 2>&1; then
        healthy=true
        break
      fi
      sleep 2
    done
    [[ $healthy == true ]] || { echo 'Synthetic MySQL startup failed.' >&2; exit 1; }
  done
  export CB_MYSQL_FIXTURE=two-empty-isolated-servers
  CB_MYSQL_SOURCE_UUID=$(docker exec "${current_pair[0]}" mysql -uroot --batch --skip-column-names -e 'SELECT @@server_uuid')
  CB_MYSQL_RESTORE_UUID=$(docker exec "${current_pair[1]}" mysql -uroot --batch --skip-column-names -e 'SELECT @@server_uuid')
  export CB_MYSQL_SOURCE_UUID CB_MYSQL_RESTORE_UUID
  # Test code independently checks both UUIDs, empty schemas, and disabled events
  # before creating synthetic data. Tests run separately with a fresh pair each.
  cargo test --locked "core::mysql_restore_tests::${tests[$index]}" -- --exact --ignored --nocapture
  for container in "${current_pair[@]}"; do
    [[ $(docker inspect --format '{{index .Config.Labels "calmbackup.ci.fixture"}}' "$container") == "$fixture_prefix" ]]
    docker stop --time 30 "$container" >/dev/null
  done
done
