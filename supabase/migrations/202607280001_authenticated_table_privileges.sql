-- RLS policies do not grant table privileges on their own. Give authenticated
-- clients only the operations already constrained by the owner policies.

grant usage on schema public to authenticated;
grant usage, select on sequence public.app_change_seq to authenticated;

grant select, insert, update, delete on table
  public.learner_profiles,
  public.problems,
  public.assets,
  public.problem_assets,
  public.schedule_states,
  public.export_snapshots,
  public.tombstones
to authenticated;

create policy review_events_reject_update on public.review_events
for update using (account_id = auth.uid()) with check (account_id = auth.uid());

create policy review_events_reject_delete on public.review_events
for delete using (account_id = auth.uid());

grant select, insert, update, delete on table public.review_events to authenticated;
grant select on table public.applied_sync_operations to authenticated;
