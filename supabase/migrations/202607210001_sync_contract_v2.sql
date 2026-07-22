-- Sync contract v2: account-wide assets, idempotent push, ordered account pull.
-- This migration is intentionally transactional. The old v1 tables are kept
-- until both replacement tables have been copied and validated.

do $$
begin
  if exists (
    select 1
    from public.assets
    where byte_length <= 0
       or plaintext_sha256 !~ '^[0-9a-f]{64}$'
       or media_type not in ('image/jpeg', 'image/png', 'image/webp')
  ) then
    raise exception 'existing assets contain values outside the v2 contract';
  end if;
end;
$$;

create temporary table _asset_id_map (
  old_id uuid primary key,
  canonical_id uuid not null
) on commit drop;

create table public.assets_v2 (
  id uuid primary key,
  account_id uuid not null references auth.users(id) on delete cascade,
  plaintext_sha256 text not null check (plaintext_sha256 ~ '^[0-9a-f]{64}$'),
  storage_object text not null,
  byte_length bigint not null check (byte_length > 0),
  media_type text not null check (media_type in ('image/jpeg', 'image/png', 'image/webp')),
  revision bigint not null check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (account_id, plaintext_sha256),
  unique (account_id, id),
  check (storage_object = account_id::text || '/' || plaintext_sha256)
);

insert into public.assets_v2 (
  id, account_id, plaintext_sha256, storage_object, byte_length, media_type,
  revision, change_seq, created_at, updated_at
)
select distinct on (a.account_id, a.plaintext_sha256)
  a.id,
  a.account_id,
  a.plaintext_sha256,
  a.account_id::text || '/' || a.plaintext_sha256,
  a.byte_length,
  a.media_type,
  a.revision,
  coalesce(a.change_seq, nextval('public.app_change_seq')),
  a.created_at,
  a.updated_at
from public.assets a
order by a.account_id, a.plaintext_sha256, a.created_at, a.id;

insert into _asset_id_map(old_id, canonical_id)
select a.id, canonical.id
from public.assets a
join public.assets_v2 canonical
  on canonical.account_id = a.account_id
 and canonical.plaintext_sha256 = a.plaintext_sha256;

create table public.problem_assets_v2 (
  id uuid primary key,
  account_id uuid not null,
  profile_id uuid not null,
  problem_id uuid not null,
  asset_id uuid not null,
  role text not null check (role in ('question', 'answer')),
  position integer not null check (position >= 0),
  revision bigint not null default 1 check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (problem_id, role, position),
  unique (problem_id, asset_id, role),
  foreign key (account_id, profile_id, problem_id)
    references public.problems(account_id, profile_id, id) on delete cascade,
  foreign key (account_id, asset_id)
    references public.assets_v2(account_id, id) on delete restrict
);

insert into public.problem_assets_v2 (
  id, account_id, profile_id, problem_id, asset_id, role, position,
  revision, change_seq, created_at, updated_at
)
select distinct on (pa.problem_id, pa.role, pa.position)
  pa.id,
  pa.account_id,
  pa.profile_id,
  pa.problem_id,
  mapping.canonical_id,
  pa.role,
  pa.position,
  pa.revision,
  coalesce(pa.change_seq, nextval('public.app_change_seq')),
  pa.created_at,
  pa.updated_at
from public.problem_assets pa
join _asset_id_map mapping on mapping.old_id = pa.asset_id
order by pa.problem_id, pa.role, pa.position, pa.created_at, pa.id;

do $$
begin
  if (select count(*) from _asset_id_map) <> (select count(*) from public.assets) then
    raise exception 'asset id mapping is incomplete';
  end if;
  if exists (
    select 1
    from public.problem_assets pa
    left join _asset_id_map mapping on mapping.old_id = pa.asset_id
    where mapping.old_id is null
  ) then
    raise exception 'problem asset references an unknown asset';
  end if;
end;
$$;

-- Remove v1 policies, triggers, and index names before the table swap.
drop policy if exists assets_owner on public.assets;
drop policy if exists problem_assets_owner on public.problem_assets;
drop trigger if exists stamp_assets on public.assets;
drop trigger if exists stamp_problem_assets on public.problem_assets;
drop index if exists public.assets_pull_idx;
drop index if exists public.problem_assets_pull_idx;

alter table public.assets rename to assets_v1;
alter table public.problem_assets rename to problem_assets_v1;
alter table public.assets_v2 rename to assets;
alter table public.problem_assets_v2 rename to problem_assets;

drop table public.problem_assets_v1;
drop table public.assets_v1;

create index assets_pull_idx on public.assets(account_id, change_seq);
create index problem_assets_pull_idx on public.problem_assets(account_id, profile_id, change_seq);

alter table public.assets enable row level security;
alter table public.problem_assets enable row level security;

create policy assets_owner on public.assets
for all using (account_id = auth.uid()) with check (account_id = auth.uid());
create policy problem_assets_owner on public.problem_assets
for all using (account_id = auth.uid()) with check (account_id = auth.uid());

create trigger stamp_assets before update on public.assets
for each row execute function public.stamp_changed_row();
create trigger stamp_problem_assets before update on public.problem_assets
for each row execute function public.stamp_changed_row();

create table public.applied_sync_operations (
  operation_id uuid not null,
  account_id uuid not null references auth.users(id) on delete cascade,
  entity_type text not null,
  entity_id uuid not null,
  change_seq bigint not null,
  applied_at timestamptz not null default now(),
  primary key (account_id, operation_id)
);

alter table public.applied_sync_operations enable row level security;
create policy applied_sync_operations_owner on public.applied_sync_operations
for select using (account_id = auth.uid());

create or replace function public.push_sync_batch(p_operations jsonb)
returns table(operation_id uuid, entity_type text, entity_id uuid, change_seq bigint)
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_account uuid := auth.uid();
  v_item jsonb;
  v_payload jsonb;
  v_operation_id uuid;
  v_entity_type text;
  v_entity_id uuid;
  v_operation text;
  v_revision bigint;
  v_change_seq bigint;
  v_row_count integer;
  v_problem_changed boolean;
  v_existing record;
  v_link jsonb;
  v_role text;
  v_position integer;
  v_asset_id uuid;
begin
  if v_account is null then
    raise exception 'authentication required' using errcode = '42501';
  end if;
  if jsonb_typeof(p_operations) <> 'array'
     or jsonb_array_length(p_operations) < 1
     or jsonb_array_length(p_operations) > 100 then
    raise exception 'operation batch must contain between 1 and 100 items' using errcode = '22023';
  end if;

  for v_item in select value from jsonb_array_elements(p_operations) loop
    if exists (
      select 1 from jsonb_object_keys(v_item) as keys(key)
      where key not in ('operationId', 'entityType', 'entityId', 'operation', 'payload')
    ) then
      raise exception 'operation contains an unknown field' using errcode = '22023';
    end if;
    v_operation_id := (v_item ->> 'operationId')::uuid;
    v_entity_type := v_item ->> 'entityType';
    v_entity_id := (v_item ->> 'entityId')::uuid;
    v_operation := v_item ->> 'operation';
    v_payload := v_item -> 'payload';
    if v_operation_id is null or v_entity_type is null or v_entity_id is null
       or jsonb_typeof(v_payload) <> 'object'
       or v_entity_type not in ('learner_profile', 'asset', 'problem', 'review_event', 'export_snapshot')
       or v_operation not in ('upsert', 'append', 'delete') then
      raise exception 'invalid sync operation' using errcode = '22023';
    end if;

    select * into v_existing
    from public.applied_sync_operations
    where account_id = v_account and applied_sync_operations.operation_id = v_operation_id;
    if found then
      if v_existing.entity_type <> v_entity_type or v_existing.entity_id <> v_entity_id then
        raise exception 'operation id was reused for a different entity' using errcode = '23505';
      end if;
      operation_id := v_existing.operation_id;
      entity_type := v_existing.entity_type;
      entity_id := v_existing.entity_id;
      change_seq := v_existing.change_seq;
      return next;
      continue;
    end if;

    if exists (select 1 from public.learner_profiles where id = v_entity_id and account_id <> v_account)
       or exists (select 1 from public.assets where id = v_entity_id and account_id <> v_account)
       or exists (select 1 from public.problems where id = v_entity_id and account_id <> v_account)
       or exists (select 1 from public.review_events where id = v_entity_id and account_id <> v_account)
       or exists (select 1 from public.export_snapshots where id = v_entity_id and account_id <> v_account)
       or exists (select 1 from public.tombstones where entity_id = v_entity_id and account_id <> v_account) then
      raise exception 'entity is not owned by the account' using errcode = '42501';
    end if;

    if v_entity_type = 'learner_profile' and v_operation = 'upsert' then
      v_revision := (v_payload ->> 'revision')::bigint;
      insert into public.learner_profiles(id, account_id, name, revision, created_at, updated_at)
      values (
        v_entity_id, v_account, left(v_payload ->> 'name', 80), v_revision,
        to_timestamp((v_payload ->> 'createdAtUtcMs')::double precision / 1000),
        to_timestamp((v_payload ->> 'updatedAtUtcMs')::double precision / 1000)
      )
      on conflict (id) do update set
        name = excluded.name,
        revision = excluded.revision,
        updated_at = excluded.updated_at
      where learner_profiles.account_id = v_account
        and excluded.revision > learner_profiles.revision;
      select lp.change_seq into v_change_seq from public.learner_profiles lp
       where lp.id = v_entity_id and lp.account_id = v_account;

    elsif v_entity_type = 'asset' and v_operation = 'upsert' then
      insert into public.assets(
        id, account_id, plaintext_sha256, storage_object, byte_length, media_type,
        revision, created_at, updated_at
      ) values (
        v_entity_id, v_account, v_payload ->> 'plaintextSha256',
        v_payload ->> 'storageObject', (v_payload ->> 'byteLength')::bigint,
        v_payload ->> 'mediaType', (v_payload ->> 'revision')::bigint,
        to_timestamp((v_payload ->> 'createdAtUtcMs')::double precision / 1000),
        to_timestamp((v_payload ->> 'createdAtUtcMs')::double precision / 1000)
      )
      on conflict (id) do update set
        storage_object = excluded.storage_object,
        byte_length = excluded.byte_length,
        media_type = excluded.media_type,
        revision = excluded.revision
      where assets.account_id = v_account
        and excluded.revision > assets.revision;
      select a.change_seq into v_change_seq from public.assets a
       where a.id = v_entity_id and a.account_id = v_account;

    elsif v_entity_type = 'problem' and v_operation = 'upsert' then
      if not exists (
        select 1 from public.learner_profiles
        where id = (v_payload ->> 'profileId')::uuid and account_id = v_account
      ) then
        raise exception 'problem profile is not owned by the account' using errcode = '42501';
      end if;
      insert into public.problems(
        id, account_id, profile_id, subject, tags, note, status,
        answer_limit_seconds, revision, created_at, updated_at
      ) values (
        v_entity_id, v_account, (v_payload ->> 'profileId')::uuid,
        left(coalesce(v_payload ->> 'subject', ''), 200),
        coalesce(v_payload -> 'tags', '[]'::jsonb), coalesce(v_payload ->> 'note', ''),
        case when v_payload ->> 'status' = 'trashed' then 'deleted' else coalesce(v_payload ->> 'status', 'active') end,
        nullif((v_payload ->> 'timeLimitSeconds')::integer, 0),
        (v_payload ->> 'revision')::bigint,
        to_timestamp((v_payload ->> 'createdAtUtcMs')::double precision / 1000),
        to_timestamp((v_payload ->> 'updatedAtUtcMs')::double precision / 1000)
      )
      on conflict (id) do update set
        subject = excluded.subject,
        tags = excluded.tags,
        note = excluded.note,
        status = excluded.status,
        answer_limit_seconds = excluded.answer_limit_seconds,
        revision = excluded.revision,
        updated_at = excluded.updated_at
      where problems.account_id = v_account
        and excluded.revision > problems.revision;

      get diagnostics v_row_count = row_count;
      v_problem_changed := v_row_count > 0;
      if jsonb_typeof(v_payload -> 'assets') <> 'array'
         or jsonb_array_length(v_payload -> 'assets') > 100 then
        raise exception 'problem asset list is invalid' using errcode = '22023';
      end if;
      if v_problem_changed then
        delete from public.problem_assets
        where problem_id = v_entity_id and account_id = v_account;
        for v_link in select value from jsonb_array_elements(v_payload -> 'assets') loop
          v_asset_id := (v_link ->> 'assetId')::uuid;
          v_role := v_link ->> 'role';
          v_position := (v_link ->> 'position')::integer;
          if v_role not in ('question', 'answer') or v_position < 0
             or not exists (select 1 from public.assets where id = v_asset_id and account_id = v_account) then
            raise exception 'problem asset is not owned by the account' using errcode = '42501';
          end if;
          insert into public.problem_assets(
            id, account_id, profile_id, problem_id, asset_id, role, position,
            revision, created_at, updated_at
          ) values (
            gen_random_uuid(), v_account, (v_payload ->> 'profileId')::uuid,
            v_entity_id, v_asset_id, v_role, v_position, 1, now(), now()
          );
        end loop;
      end if;
      select p.change_seq into v_change_seq from public.problems p
       where p.id = v_entity_id and p.account_id = v_account;

    elsif v_entity_type = 'review_event' and v_operation in ('upsert', 'append') then
      if not exists (
        select 1 from public.problems
        where id = (v_payload ->> 'problemId')::uuid and account_id = v_account
      ) then
        raise exception 'review problem is not owned by the account' using errcode = '42501';
      end if;
      insert into public.review_events(
        id, account_id, profile_id, problem_id, device_id, rating, duration_ms,
        occurred_at, algorithm_version, parameter_version
      ) values (
        v_entity_id, v_account, (v_payload ->> 'profileId')::uuid,
        (v_payload ->> 'problemId')::uuid, (v_payload ->> 'deviceId')::uuid,
        v_payload ->> 'rating', (v_payload ->> 'durationMs')::integer,
        to_timestamp((v_payload ->> 'occurredAtUtcMs')::double precision / 1000),
        left(v_payload ->> 'algorithmVersion', 80), left(v_payload ->> 'parameterVersion', 80)
      ) on conflict (id) do nothing;
      select r.change_seq into v_change_seq from public.review_events r
       where r.id = v_entity_id and r.account_id = v_account;

    elsif v_entity_type = 'export_snapshot' and v_operation = 'upsert' then
      insert into public.export_snapshots(
        id, account_id, profile_id, name, selection, configuration,
        revision, created_at, updated_at
      ) values (
        v_entity_id, v_account, (v_payload ->> 'profileId')::uuid,
        left(v_payload ->> 'title', 120), coalesce(v_payload -> 'problemIds', '[]'::jsonb),
        coalesce(v_payload -> 'configuration', '{}'::jsonb), (v_payload ->> 'revision')::bigint,
        to_timestamp((v_payload ->> 'createdAtUtcMs')::double precision / 1000),
        to_timestamp((v_payload ->> 'createdAtUtcMs')::double precision / 1000)
      ) on conflict (id) do update set
        name = excluded.name,
        selection = excluded.selection,
        configuration = excluded.configuration,
        revision = excluded.revision
      where export_snapshots.account_id = v_account
        and excluded.revision > export_snapshots.revision;
      select e.change_seq into v_change_seq from public.export_snapshots e
       where e.id = v_entity_id and e.account_id = v_account;

    elsif v_entity_type in ('learner_profile', 'problem', 'asset', 'export_snapshot')
      and v_operation = 'delete' then
      insert into public.tombstones(
        id, account_id, profile_id, entity_type, entity_id,
        deleted_revision, purge_after, revision
      ) values (
        (v_payload ->> 'tombstoneId')::uuid, v_account,
        nullif(v_payload ->> 'profileId', '')::uuid, v_entity_type, v_entity_id,
        (v_payload ->> 'deletedRevision')::bigint,
        to_timestamp((v_payload ->> 'purgeAfterUtcMs')::double precision / 1000), 1
      ) on conflict (account_id, profile_id, entity_type, entity_id) do update set
        deleted_revision = excluded.deleted_revision,
        purge_after = excluded.purge_after,
        revision = excluded.revision
      where tombstones.account_id = v_account
        and excluded.deleted_revision > tombstones.deleted_revision;
      if v_entity_type = 'problem' then
        update public.problems set status = 'deleted'
        where id = v_entity_id and account_id = v_account;
      end if;
      select t.change_seq into v_change_seq from public.tombstones t
       where t.entity_id = v_entity_id and t.entity_type = v_entity_type and t.account_id = v_account;
    else
      raise exception 'operation is not valid for this entity' using errcode = '22023';
    end if;

    v_change_seq := coalesce(v_change_seq, 0);
    insert into public.applied_sync_operations(operation_id, account_id, entity_type, entity_id, change_seq)
    values (v_operation_id, v_account, v_entity_type, v_entity_id, v_change_seq);
    operation_id := v_operation_id;
    entity_type := v_entity_type;
    entity_id := v_entity_id;
    change_seq := v_change_seq;
    return next;
  end loop;
end;
$$;

revoke all on function public.push_sync_batch(jsonb) from public;
grant execute on function public.push_sync_batch(jsonb) to authenticated;

create index if not exists learner_profiles_account_change_idx
  on public.learner_profiles(account_id, change_seq);

create or replace function public.pull_account_changes(
  p_after bigint default 0,
  p_limit integer default 500
)
returns table(change_seq bigint, entity_type text, entity_id uuid, operation text, payload jsonb)
language sql
stable
security invoker
set search_path = ''
as $$
  select feed.change_seq, feed.entity_type, feed.entity_id, feed.operation, feed.payload
  from (
    select p.change_seq, 'learner_profile'::text, p.id, 'upsert'::text,
      jsonb_build_object('accountId', p.account_id, 'id', p.id, 'name', p.name,
        'revision', p.revision,
        'createdAtUtcMs', floor(extract(epoch from p.created_at) * 1000)::bigint,
        'updatedAtUtcMs', floor(extract(epoch from p.updated_at) * 1000)::bigint)
    from public.learner_profiles p
    where p.account_id = auth.uid() and p.change_seq > p_after
    union all
    select a.change_seq, 'asset'::text, a.id, 'upsert'::text,
      jsonb_build_object('accountId', a.account_id, 'id', a.id,
        'plaintextSha256', a.plaintext_sha256, 'storageObject', a.storage_object,
        'byteLength', a.byte_length, 'mediaType', a.media_type, 'revision', a.revision,
        'createdAtUtcMs', floor(extract(epoch from a.created_at) * 1000)::bigint)
    from public.assets a
    where a.account_id = auth.uid() and a.change_seq > p_after
    union all
    select p.change_seq, 'problem'::text, p.id, 'upsert'::text,
      jsonb_build_object('accountId', p.account_id, 'id', p.id, 'profileId', p.profile_id,
        'subject', p.subject, 'tags', p.tags, 'note', p.note,
        'status', case when p.status = 'deleted' then 'trashed' else p.status end,
        'timeLimitSeconds', p.answer_limit_seconds,
        'assets', coalesce((select jsonb_agg(jsonb_build_object(
          'assetId', pa.asset_id, 'role', pa.role, 'position', pa.position)
          order by pa.role, pa.position
          from public.problem_assets pa
          where pa.account_id = p.account_id and pa.problem_id = p.id), '[]'::jsonb),
        'revision', p.revision,
        'createdAtUtcMs', floor(extract(epoch from p.created_at) * 1000)::bigint,
        'updatedAtUtcMs', floor(extract(epoch from p.updated_at) * 1000)::bigint)
    from public.problems p
    where p.account_id = auth.uid() and p.change_seq > p_after
    union all
    select r.change_seq, 'review_event'::text, r.id, 'append'::text,
      jsonb_build_object('accountId', r.account_id, 'id', r.id, 'profileId', r.profile_id,
        'problemId', r.problem_id, 'deviceId', r.device_id, 'rating', r.rating,
        'durationMs', r.duration_ms,
        'occurredAtUtcMs', floor(extract(epoch from r.occurred_at) * 1000)::bigint,
        'algorithmVersion', r.algorithm_version, 'parameterVersion', r.parameter_version)
    from public.review_events r
    where r.account_id = auth.uid() and r.change_seq > p_after
    union all
    select e.change_seq, 'export_snapshot'::text, e.id, 'upsert'::text,
      jsonb_build_object('accountId', e.account_id, 'id', e.id, 'profileId', e.profile_id,
        'title', e.name, 'problemIds', e.selection, 'configuration', e.configuration,
        'revision', e.revision,
        'createdAtUtcMs', floor(extract(epoch from e.created_at) * 1000)::bigint)
    from public.export_snapshots e
    where e.account_id = auth.uid() and e.change_seq > p_after
    union all
    select t.change_seq, t.entity_type, t.entity_id, 'delete'::text,
      jsonb_build_object('accountId', t.account_id, 'tombstoneId', t.id,
        'profileId', t.profile_id, 'entityType', t.entity_type, 'entityId', t.entity_id,
        'deletedAtUtcMs', floor(extract(epoch from t.created_at) * 1000)::bigint,
        'purgeAfterUtcMs', floor(extract(epoch from t.purge_after) * 1000)::bigint,
        'deletedRevision', t.deleted_revision)
    from public.tombstones t
    where t.account_id = auth.uid() and t.change_seq > p_after
  ) as feed(change_seq, entity_type, entity_id, operation, payload)
  where feed.entity_type in ('learner_profile', 'asset', 'problem', 'review_event', 'export_snapshot')
  order by feed.change_seq, feed.entity_type, feed.entity_id
  limit least(greatest(coalesce(p_limit, 500), 1), 500);
$$;

revoke all on function public.pull_account_changes(bigint, integer) from public;
grant execute on function public.pull_account_changes(bigint, integer) to authenticated;
