begin;

select plan(10);

insert into auth.users (id, email, aud, role)
values ('33333333-3333-4333-8333-333333333333', 'contract@example.test', 'authenticated', 'authenticated');

insert into auth.users (id, email, aud, role)
values ('11111111-1111-4111-8111-111111111111', 'foreign@example.test', 'authenticated', 'authenticated');

set local role authenticated;
set local "request.jwt.claim.sub" = '33333333-3333-4333-8333-333333333333';

insert into public.learner_profiles (id, account_id, name)
values ('cccccccc-cccc-4ccc-8ccc-cccccccccccc', '33333333-3333-4333-8333-333333333333', '契约档案');

insert into public.problems (id, account_id, profile_id, subject)
values (
  'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
  '33333333-3333-4333-8333-333333333333',
  'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
  '数学'
);

select results_eq(
  $$select count(*)::bigint from public.pull_account_changes(0, 500)$$,
  $$values (2::bigint)$$,
  'the account feed includes the profile and problem for the signed-in account'
);

select is(
  (select (payload ->> 'accountId')::uuid
     from public.pull_account_changes(0, 500)
    where entity_type = 'learner_profile'),
  '33333333-3333-4333-8333-333333333333'::uuid,
  'account identity is explicit in every wire payload'
);

select is(
  (select payload ->> 'profileId'
     from public.pull_account_changes(0, 500)
    where entity_type = 'problem'),
  'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
  'problem payload uses the camelCase profileId field'
);

set local "request.jwt.claim.sub" = '11111111-1111-4111-8111-111111111111';

select results_eq(
  $$select count(*)::bigint from public.pull_account_changes(0, 500)$$,
  $$values (0::bigint)$$,
  'the account feed never exposes another account'
);

insert into public.learner_profiles (id, account_id, name)
values ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', '11111111-1111-4111-8111-111111111111', 'foreign profile');

set local "request.jwt.claim.sub" = '33333333-3333-4333-8333-333333333333';

select is(
  (select count(*)::bigint from public.push_sync_batch(jsonb_build_array(
    jsonb_build_object(
      'operationId', 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
      'entityType', 'learner_profile',
      'entityId', 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
      'operation', 'upsert',
      'payload', jsonb_build_object(
        'id', 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
        'name', '契约档案更新',
        'revision', 2,
        'createdAtUtcMs', 10,
        'updatedAtUtcMs', 20
      )
    )
  ))),
  1::bigint,
  'one owned profile operation is acknowledged'
);

select is(
  (select count(*)::bigint from public.push_sync_batch(jsonb_build_array(
    jsonb_build_object(
      'operationId', 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
      'entityType', 'learner_profile',
      'entityId', 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
      'operation', 'upsert',
      'payload', jsonb_build_object('id', 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', 'name', '不应重复应用', 'revision', 99)
    )
  ))),
  1::bigint,
  'replaying an operation id returns one stored acknowledgement'
);

select is(
  (select name from public.learner_profiles where id = 'cccccccc-cccc-4ccc-8ccc-cccccccccccc'),
  '契约档案更新',
  'replaying an operation does not rewrite the canonical row'
);

select is(
  (select count(*)::bigint from public.push_sync_batch(jsonb_build_array(
    jsonb_build_object(
      'operationId', 'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
      'entityType', 'problem',
      'entityId', 'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
      'operation', 'delete',
      'payload', jsonb_build_object(
        'tombstoneId', 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
        'profileId', 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
        'entityType', 'problem',
        'entityId', 'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
        'deletedRevision', 1,
        'purgeAfterUtcMs', 31536000000
      )
    )
  ))),
  1::bigint,
  'problem tombstones use the account/profile composite conflict key'
);

select throws_ok(
  $$select * from public.push_sync_batch(null::jsonb)$$,
  '22023',
  'operation batch must contain between 1 and 100 items',
  'null batches are rejected instead of being treated as empty'
);

select throws_ok(
  $$select * from public.push_sync_batch(jsonb_build_array(
    jsonb_build_object(
      'operationId', 'ffffffff-ffff-4fff-8fff-ffffffffffff',
      'entityType', 'learner_profile',
      'entityId', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      'operation', 'upsert',
      'payload', jsonb_build_object('id', 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', 'name', '越权档案', 'revision', 1)
    )
  ))$$,
  '42501',
  'entity is not owned by the account',
  'push rejects a row owned by another account'
);

select * from finish();
rollback;
