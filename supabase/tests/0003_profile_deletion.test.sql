begin;

select plan(10);

insert into auth.users (id, email, aud, role)
values ('44444444-4444-4444-8444-444444444444', 'profile-delete@example.test', 'authenticated', 'authenticated');

set local role authenticated;
set local "request.jwt.claim.sub" = '44444444-4444-4444-8444-444444444444';

insert into public.learner_profiles (id, account_id, name)
values
  ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1', '44444444-4444-4444-8444-444444444444', '待删除'),
  ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2', '44444444-4444-4444-8444-444444444444', '保留档案');

insert into public.assets (
  id, account_id, plaintext_sha256, storage_object, byte_length, media_type, revision
) values
  ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1', '44444444-4444-4444-8444-444444444444', repeat('1', 64), '44444444-4444-4444-8444-444444444444/' || repeat('1', 64), 10, 'image/png', 1),
  ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2', '44444444-4444-4444-8444-444444444444', repeat('2', 64), '44444444-4444-4444-8444-444444444444/' || repeat('2', 64), 10, 'image/png', 1);

insert into public.problems (id, account_id, profile_id, subject)
values
  ('cccccccc-cccc-4ccc-8ccc-ccccccccccc1', '44444444-4444-4444-8444-444444444444', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1', '数学'),
  ('cccccccc-cccc-4ccc-8ccc-ccccccccccc2', '44444444-4444-4444-8444-444444444444', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2', '数学');

insert into public.problem_assets (
  id, account_id, profile_id, problem_id, asset_id, role, position
) values
  ('dddddddd-dddd-4ddd-8ddd-ddddddddddd1', '44444444-4444-4444-8444-444444444444', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1', 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1', 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1', 'question', 0),
  ('dddddddd-dddd-4ddd-8ddd-ddddddddddd2', '44444444-4444-4444-8444-444444444444', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1', 'cccccccc-cccc-4ccc-8ccc-ccccccccccc1', 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2', 'answer', 0),
  ('dddddddd-dddd-4ddd-8ddd-ddddddddddd3', '44444444-4444-4444-8444-444444444444', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2', 'cccccccc-cccc-4ccc-8ccc-ccccccccccc2', 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1', 'question', 0);

insert into public.review_events (
  id, account_id, profile_id, problem_id, device_id, rating, duration_ms,
  occurred_at, algorithm_version, parameter_version
) values (
  '12121212-1212-4212-8212-121212121212',
  '44444444-4444-4444-8444-444444444444',
  'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
  'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
  '13131313-1313-4313-8313-131313131313',
  'good', 1200, now(), 'fsrs-6', 'default-1'
);

insert into public.schedule_states (
  id, account_id, profile_id, problem_id, due_at, stability, difficulty,
  last_review_event_id, algorithm_version, parameter_version
) values (
  '14141414-1414-4414-8414-141414141414',
  '44444444-4444-4444-8444-444444444444',
  'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
  'cccccccc-cccc-4ccc-8ccc-ccccccccccc1',
  now(), 1, 5, '12121212-1212-4212-8212-121212121212', 'fsrs-6', 'default-1'
);

insert into public.export_snapshots (
  id, account_id, profile_id, name, selection, configuration
) values (
  '15151515-1515-4515-8515-151515151515',
  '44444444-4444-4444-8444-444444444444',
  'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
  'å¾…åˆ é™¤å¯¼å‡º', '[]'::jsonb, '{}'::jsonb
);

select is(
  (select count(*)::bigint from public.push_sync_batch(jsonb_build_array(
    jsonb_build_object(
      'operationId', 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee1',
      'entityType', 'learner_profile',
      'entityId', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
      'operation', 'delete',
      'payload', jsonb_build_object(
        'tombstoneId', 'ffffffff-ffff-4fff-8fff-fffffffffff1',
        'profileId', null,
        'entityType', 'learner_profile',
        'entityId', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
        'deletedRevision', 2,
        'purgeAfterUtcMs', 32503680000000
      )
    )
  ))),
  1::bigint,
  'profile deletion is acknowledged exactly once'
);

select is(
  (select count(*)::bigint from public.learner_profiles where id = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1'),
  0::bigint,
  'the canonical profile and its cascaded rows are removed'
);

select is(
  (select count(*)::bigint from public.review_events where profile_id = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1'),
  0::bigint,
  'profile deletion cascades immutable review history through the authorized foreign key'
);

select is(
  (select count(*)::bigint from public.schedule_states where profile_id = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1'),
  0::bigint,
  'profile deletion removes schedule state'
);

select is(
  (select count(*)::bigint from public.export_snapshots where profile_id = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1'),
  0::bigint,
  'profile deletion removes export snapshots'
);

select is(
  (select count(*)::bigint from public.assets where id = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb1'),
  1::bigint,
  'an asset shared by the remaining profile is preserved'
);

select is(
  (select count(*)::bigint from public.assets where id = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2'),
  0::bigint,
  'an asset orphaned by profile deletion is removed'
);

select is(
  (select count(*)::bigint from public.tombstones
   where entity_type = 'asset' and entity_id = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb2' and profile_id is null),
  1::bigint,
  'orphan cleanup creates an account-scoped asset tombstone'
);

select throws_ok(
  $$select * from public.push_sync_batch(jsonb_build_array(
    jsonb_build_object(
      'operationId', 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee2',
      'entityType', 'learner_profile',
      'entityId', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2',
      'operation', 'delete',
      'payload', jsonb_build_object(
        'tombstoneId', 'ffffffff-ffff-4fff-8fff-fffffffffff2',
        'profileId', null,
        'entityType', 'learner_profile',
        'entityId', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa2',
        'deletedRevision', 2,
        'purgeAfterUtcMs', 32503680000000
      )
    )
  ))$$,
  '23514',
  'the last learner profile cannot be deleted',
  'the remote contract also rejects deleting the last profile'
);

select throws_ok(
  $$select * from public.push_sync_batch(jsonb_build_array(
    jsonb_build_object(
      'operationId', 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeee3',
      'entityType', 'learner_profile',
      'entityId', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1',
      'operation', 'upsert',
      'payload', jsonb_build_object(
        'name', 'æ—§è®¾å¤‡æ¡£æ¡ˆ',
        'revision', 99,
        'createdAtUtcMs', 1000,
        'updatedAtUtcMs', 2000
      )
    )
  ))$$,
  '23514',
  'deleted learner profile cannot be restored by a stale upsert',
  'a retained profile tombstone prevents stale devices from resurrecting the profile'
);

select * from finish();
rollback;
