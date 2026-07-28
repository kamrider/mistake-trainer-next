-- Account-scoped profile and asset tombstones must survive profile cascade.
-- Applying one atomically removes the canonical remote rows while preserving
-- assets that are still referenced by another learner profile.

alter table public.tombstones
  drop constraint if exists tombstones_account_id_profile_id_fkey;
alter table public.tombstones
  drop constraint if exists tombstones_account_id_profile_id_entity_type_entity_id_key;
alter table public.tombstones
  alter column profile_id drop not null;

-- v2 allowed asset tombstones to retain the profile that first referenced the
-- asset. Normalize retained rows before enforcing the account-scoped v3 shape.
delete from public.tombstones target
using (
  select id
  from (
    select id, row_number() over (
      partition by account_id, entity_type, entity_id
      order by deleted_revision desc, change_seq desc, id
    ) as ordinal
    from public.tombstones
    where entity_type in ('learner_profile', 'asset')
  ) ranked
  where ranked.ordinal > 1
) duplicate
where target.id = duplicate.id;

update public.tombstones
set profile_id = null
where entity_type in ('learner_profile', 'asset') and profile_id is not null;

alter table public.tombstones
  drop constraint if exists tombstones_profile_scope;
alter table public.tombstones
  drop constraint if exists tombstones_entity_identity;
alter table public.tombstones
  add constraint tombstones_profile_scope check (
    (entity_type in ('learner_profile', 'asset') and profile_id is null)
    or (entity_type not in ('learner_profile', 'asset') and profile_id is not null)
  );
alter table public.tombstones
  add constraint tombstones_entity_identity
  unique nulls not distinct (account_id, profile_id, entity_type, entity_id);

-- Review events stay append-only to authenticated clients, while deletion of
-- their owning problem/profile is an authorized database cascade.
drop trigger if exists prevent_review_event_mutation on public.review_events;
create trigger prevent_review_event_mutation
before update on public.review_events
for each row execute function public.prevent_review_event_mutation();

alter table public.review_events
  drop constraint if exists review_events_account_id_profile_id_problem_id_fkey;
alter table public.review_events
  add constraint review_events_account_id_profile_id_problem_id_fkey
  foreign key (account_id, profile_id, problem_id)
  references public.problems(account_id, profile_id, id) on delete cascade;

create or replace function public.prevent_deleted_profile_resurrection()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  if exists (
    select 1
    from public.tombstones t
    where t.account_id = new.account_id
      and t.profile_id is null
      and t.entity_type = 'learner_profile'
      and t.entity_id = new.id
      and t.purge_after > now()
  ) then
    raise exception 'deleted learner profile cannot be restored by a stale upsert'
      using errcode = '23514';
  end if;
  return new;
end;
$$;

drop trigger if exists prevent_deleted_profile_resurrection on public.learner_profiles;
create trigger prevent_deleted_profile_resurrection
before insert or update on public.learner_profiles
for each row execute function public.prevent_deleted_profile_resurrection();

create or replace function public.apply_delete_tombstone()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
declare
  v_candidate_assets uuid[] := '{}'::uuid[];
  v_asset_id uuid;
  v_asset_revision bigint;
begin
  if new.entity_type = 'learner_profile' then
    if new.profile_id is not null then
      raise exception 'profile tombstones must be account scoped' using errcode = '22023';
    end if;
    -- Serialize deletions for this account before checking the last-profile
    -- invariant. Without row locks, two concurrent deletes could both observe
    -- two profiles and leave the account with none.
    perform 1
    from public.learner_profiles p
    where p.account_id = new.account_id
    order by p.id
    for update;
    if exists (
      select 1 from public.learner_profiles p
      where p.id = new.entity_id and p.account_id = new.account_id
    ) then
      if (select count(*) from public.learner_profiles p where p.account_id = new.account_id) <= 1 then
        raise exception 'the last learner profile cannot be deleted' using errcode = '23514';
      end if;
      select coalesce(array_agg(distinct pa.asset_id), '{}'::uuid[])
        into v_candidate_assets
      from public.problem_assets pa
      where pa.account_id = new.account_id and pa.profile_id = new.entity_id;

      delete from public.learner_profiles p
      where p.id = new.entity_id and p.account_id = new.account_id;

      foreach v_asset_id in array v_candidate_assets loop
        if not exists (
          select 1 from public.problem_assets pa
          where pa.account_id = new.account_id and pa.asset_id = v_asset_id
        ) then
          select a.revision into v_asset_revision
          from public.assets a
          where a.id = v_asset_id and a.account_id = new.account_id;
          if found then
            insert into public.tombstones(
              id, account_id, profile_id, entity_type, entity_id,
              deleted_revision, purge_after, revision
            ) values (
              gen_random_uuid(), new.account_id, null, 'asset', v_asset_id,
              greatest(v_asset_revision + 1, 1), new.purge_after, 1
            ) on conflict (account_id, profile_id, entity_type, entity_id) do update set
              deleted_revision = greatest(public.tombstones.deleted_revision, excluded.deleted_revision),
              purge_after = greatest(public.tombstones.purge_after, excluded.purge_after),
              revision = public.tombstones.revision + 1;
          end if;
        end if;
      end loop;
    end if;
  elsif new.entity_type = 'asset' then
    if new.profile_id is not null then
      raise exception 'asset tombstones must be account scoped' using errcode = '22023';
    end if;
    delete from public.assets a
    where a.id = new.entity_id and a.account_id = new.account_id
      and not exists (
        select 1 from public.problem_assets pa
        where pa.account_id = new.account_id and pa.asset_id = new.entity_id
      );
  end if;
  return new;
end;
$$;

drop trigger if exists apply_delete_tombstone on public.tombstones;
create trigger apply_delete_tombstone
after insert or update on public.tombstones
for each row execute function public.apply_delete_tombstone();
