create sequence public.app_change_seq as bigint;

create table public.learner_profiles (
  id uuid primary key,
  account_id uuid not null references auth.users(id) on delete cascade,
  name text not null check (length(btrim(name)) between 1 and 80),
  revision bigint not null default 1 check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (account_id, id),
  unique (account_id, name)
);

create table public.problems (
  id uuid primary key,
  account_id uuid not null,
  profile_id uuid not null,
  subject text not null default '',
  tags jsonb not null default '[]'::jsonb check (jsonb_typeof(tags) = 'array'),
  note text not null default '',
  status text not null default 'active' check (status in ('active', 'archived', 'deleted')),
  answer_limit_seconds integer check (answer_limit_seconds is null or answer_limit_seconds > 0),
  revision bigint not null default 1 check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (account_id, profile_id, id),
  foreign key (account_id, profile_id)
    references public.learner_profiles(account_id, id) on delete cascade
);

create table public.assets (
  id uuid primary key,
  account_id uuid not null,
  profile_id uuid not null,
  plaintext_sha256 text not null check (plaintext_sha256 ~ '^[0-9a-f]{64}$'),
  storage_object text not null,
  byte_length bigint not null check (byte_length > 0),
  media_type text not null check (media_type like 'image/%'),
  revision bigint not null default 1 check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (account_id, profile_id, id),
  unique (account_id, profile_id, plaintext_sha256),
  foreign key (account_id, profile_id)
    references public.learner_profiles(account_id, id) on delete cascade,
  check (storage_object like account_id::text || '/%')
);

create table public.problem_assets (
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
  foreign key (account_id, profile_id, asset_id)
    references public.assets(account_id, profile_id, id) on delete restrict
);

create table public.review_events (
  id uuid primary key,
  account_id uuid not null,
  profile_id uuid not null,
  problem_id uuid not null,
  device_id uuid not null,
  rating text not null check (rating in ('again', 'hard', 'good', 'easy')),
  duration_ms integer not null check (duration_ms >= 0),
  occurred_at timestamptz not null,
  algorithm_version text not null,
  parameter_version text not null,
  revision bigint not null default 1 check (revision = 1),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  unique (account_id, profile_id, id),
  foreign key (account_id, profile_id, problem_id)
    references public.problems(account_id, profile_id, id) on delete restrict
);

create table public.schedule_states (
  id uuid primary key,
  account_id uuid not null,
  profile_id uuid not null,
  problem_id uuid not null,
  due_at timestamptz not null,
  stability double precision not null check (stability >= 0),
  difficulty double precision not null check (difficulty between 1 and 10),
  last_review_event_id uuid,
  algorithm_version text not null,
  parameter_version text not null,
  revision bigint not null default 1 check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (account_id, profile_id, problem_id),
  foreign key (account_id, profile_id, problem_id)
    references public.problems(account_id, profile_id, id) on delete cascade,
  foreign key (last_review_event_id)
    references public.review_events(id) on delete set null
);

create table public.export_snapshots (
  id uuid primary key,
  account_id uuid not null,
  profile_id uuid not null,
  name text not null check (length(btrim(name)) between 1 and 120),
  selection jsonb not null,
  configuration jsonb not null,
  revision bigint not null default 1 check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (account_id, profile_id, id),
  foreign key (account_id, profile_id)
    references public.learner_profiles(account_id, id) on delete cascade
);

create table public.tombstones (
  id uuid primary key,
  account_id uuid not null,
  profile_id uuid not null,
  entity_type text not null,
  entity_id uuid not null,
  deleted_revision bigint not null check (deleted_revision > 0),
  purge_after timestamptz not null default (now() + interval '30 days'),
  revision bigint not null default 1 check (revision > 0),
  change_seq bigint not null default nextval('public.app_change_seq'),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (account_id, profile_id, entity_type, entity_id),
  foreign key (account_id, profile_id)
    references public.learner_profiles(account_id, id) on delete cascade
);

create index problems_pull_idx on public.problems(account_id, profile_id, change_seq);
create index assets_pull_idx on public.assets(account_id, profile_id, change_seq);
create index problem_assets_pull_idx on public.problem_assets(account_id, profile_id, change_seq);
create index review_events_pull_idx on public.review_events(account_id, profile_id, change_seq);
create index schedule_states_pull_idx on public.schedule_states(account_id, profile_id, change_seq);
create index export_snapshots_pull_idx on public.export_snapshots(account_id, profile_id, change_seq);
create index tombstones_pull_idx on public.tombstones(account_id, profile_id, change_seq);

create function public.stamp_changed_row()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
  new.revision := old.revision + 1;
  new.change_seq := nextval('public.app_change_seq');
  new.updated_at := now();
  return new;
end;
$$;

create trigger stamp_learner_profiles before update on public.learner_profiles
for each row execute function public.stamp_changed_row();
create trigger stamp_problems before update on public.problems
for each row execute function public.stamp_changed_row();
create trigger stamp_assets before update on public.assets
for each row execute function public.stamp_changed_row();
create trigger stamp_problem_assets before update on public.problem_assets
for each row execute function public.stamp_changed_row();
create trigger stamp_schedule_states before update on public.schedule_states
for each row execute function public.stamp_changed_row();
create trigger stamp_export_snapshots before update on public.export_snapshots
for each row execute function public.stamp_changed_row();
create trigger stamp_tombstones before update on public.tombstones
for each row execute function public.stamp_changed_row();

create function public.prevent_review_event_mutation()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
  raise exception 'review events are immutable' using errcode = '55000';
end;
$$;

create trigger prevent_review_event_mutation
before update or delete on public.review_events
for each row execute function public.prevent_review_event_mutation();

alter table public.learner_profiles enable row level security;
alter table public.problems enable row level security;
alter table public.assets enable row level security;
alter table public.problem_assets enable row level security;
alter table public.review_events enable row level security;
alter table public.schedule_states enable row level security;
alter table public.export_snapshots enable row level security;
alter table public.tombstones enable row level security;

create policy learner_profiles_owner on public.learner_profiles
for all using (account_id = auth.uid()) with check (account_id = auth.uid());
create policy problems_owner on public.problems
for all using (account_id = auth.uid()) with check (account_id = auth.uid());
create policy assets_owner on public.assets
for all using (account_id = auth.uid()) with check (account_id = auth.uid());
create policy problem_assets_owner on public.problem_assets
for all using (account_id = auth.uid()) with check (account_id = auth.uid());
create policy review_events_read on public.review_events
for select using (account_id = auth.uid());
create policy review_events_append on public.review_events
for insert with check (account_id = auth.uid());
create policy schedule_states_owner on public.schedule_states
for all using (account_id = auth.uid()) with check (account_id = auth.uid());
create policy export_snapshots_owner on public.export_snapshots
for all using (account_id = auth.uid()) with check (account_id = auth.uid());
create policy tombstones_owner on public.tombstones
for all using (account_id = auth.uid()) with check (account_id = auth.uid());

create function public.pull_profile_changes(
  p_profile_id uuid,
  p_after bigint default 0,
  p_limit integer default 500
)
returns table(change_seq bigint, entity_type text, entity_id uuid, operation text, payload jsonb)
language sql
stable
security invoker
set search_path = ''
as $$
  select changes.change_seq, changes.entity_type, changes.entity_id, changes.operation, changes.payload
  from (
    select p.change_seq, 'problem'::text, p.id, 'upsert'::text, to_jsonb(p) from public.problems p
      where p.account_id = auth.uid() and p.profile_id = p_profile_id and p.change_seq > p_after
    union all
    select a.change_seq, 'asset'::text, a.id, 'upsert'::text, to_jsonb(a) from public.assets a
      where a.account_id = auth.uid() and a.profile_id = p_profile_id and a.change_seq > p_after
    union all
    select pa.change_seq, 'problem_asset'::text, pa.id, 'upsert'::text, to_jsonb(pa) from public.problem_assets pa
      where pa.account_id = auth.uid() and pa.profile_id = p_profile_id and pa.change_seq > p_after
    union all
    select r.change_seq, 'review_event'::text, r.id, 'append'::text, to_jsonb(r) from public.review_events r
      where r.account_id = auth.uid() and r.profile_id = p_profile_id and r.change_seq > p_after
    union all
    select s.change_seq, 'schedule_state'::text, s.id, 'upsert'::text, to_jsonb(s) from public.schedule_states s
      where s.account_id = auth.uid() and s.profile_id = p_profile_id and s.change_seq > p_after
    union all
    select e.change_seq, 'export_snapshot'::text, e.id, 'upsert'::text, to_jsonb(e) from public.export_snapshots e
      where e.account_id = auth.uid() and e.profile_id = p_profile_id and e.change_seq > p_after
    union all
    select t.change_seq, t.entity_type, t.entity_id, 'delete'::text, to_jsonb(t) from public.tombstones t
      where t.account_id = auth.uid() and t.profile_id = p_profile_id and t.change_seq > p_after
  ) as changes(change_seq, entity_type, entity_id, operation, payload)
  order by changes.change_seq
  limit least(greatest(p_limit, 1), 1000);
$$;

revoke all on function public.pull_profile_changes(uuid, bigint, integer) from public;
grant execute on function public.pull_profile_changes(uuid, bigint, integer) to authenticated;

insert into storage.buckets (id, name, public)
values ('mistake-assets', 'mistake-assets', false)
on conflict (id) do update set public = false;

create policy mistake_assets_read on storage.objects
for select to authenticated
using (
  bucket_id = 'mistake-assets'
  and (storage.foldername(name))[1] = auth.uid()::text
);
create policy mistake_assets_insert on storage.objects
for insert to authenticated
with check (
  bucket_id = 'mistake-assets'
  and (storage.foldername(name))[1] = auth.uid()::text
);
create policy mistake_assets_update on storage.objects
for update to authenticated
using (
  bucket_id = 'mistake-assets'
  and (storage.foldername(name))[1] = auth.uid()::text
)
with check (
  bucket_id = 'mistake-assets'
  and (storage.foldername(name))[1] = auth.uid()::text
);
create policy mistake_assets_delete on storage.objects
for delete to authenticated
using (
  bucket_id = 'mistake-assets'
  and (storage.foldername(name))[1] = auth.uid()::text
);
