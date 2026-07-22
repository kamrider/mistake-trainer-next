-- Account-scoped profile and asset tombstones must survive profile cascade.
-- Applying one atomically removes the canonical remote rows while preserving
-- assets that are still referenced by another learner profile.

alter table public.tombstones
  drop constraint if exists tombstones_account_id_profile_id_fkey;
alter table public.tombstones
  drop constraint if exists tombstones_account_id_profile_id_entity_type_entity_id_key;
alter table public.tombstones
  alter column profile_id drop not null;
alter table public.tombstones
  add constraint tombstones_profile_scope check (
    (entity_type in ('learner_profile', 'asset') and profile_id is null)
    or (entity_type not in ('learner_profile', 'asset') and profile_id is not null)
  );
alter table public.tombstones
  add constraint tombstones_entity_identity
  unique nulls not distinct (account_id, profile_id, entity_type, entity_id);

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

