begin;

create extension if not exists pgtap with schema extensions;
select plan(6);

insert into auth.users (id, email, aud, role)
values
  ('11111111-1111-4111-8111-111111111111', 'first@example.test', 'authenticated', 'authenticated'),
  ('22222222-2222-4222-8222-222222222222', 'second@example.test', 'authenticated', 'authenticated');

insert into public.learner_profiles (id, account_id, name)
values
  ('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', '11111111-1111-4111-8111-111111111111', '甲档案'),
  ('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', '22222222-2222-4222-8222-222222222222', '乙档案');

set local role authenticated;
set local "request.jwt.claim.sub" = '11111111-1111-4111-8111-111111111111';

select results_eq(
  $$select name from public.learner_profiles order by name$$,
  array['甲档案'::text],
  'an account only reads its own profiles'
);

select throws_ok(
  $$insert into public.learner_profiles (id, account_id, name)
    values ('cccccccc-cccc-4ccc-8ccc-cccccccccccc',
            '22222222-2222-4222-8222-222222222222', '越权档案')$$,
  '42501',
  'new row violates row-level security policy for table "learner_profiles"',
  'an account cannot insert for another account'
);

select lives_ok(
  $$insert into public.problems (id, account_id, profile_id, subject)
    values ('dddddddd-dddd-4ddd-8ddd-dddddddddddd',
            '11111111-1111-4111-8111-111111111111',
            'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', '数学')$$,
  'an account can insert a problem into its own profile'
);

select lives_ok(
  $$insert into public.review_events (
      id, account_id, profile_id, problem_id, device_id, rating,
      duration_ms, occurred_at, algorithm_version, parameter_version
    ) values (
      'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
      '11111111-1111-4111-8111-111111111111',
      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
      'ffffffff-ffff-4fff-8fff-ffffffffffff', 'good', 1200, now(),
      'fsrs-6.6.1', 'default-6.6.1'
    )$$,
  'review events can be appended'
);

select throws_ok(
  $$update public.review_events set rating = 'again'
    where id = 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee'$$,
  '55000',
  'review events are immutable',
  'review events cannot be rewritten'
);

select throws_ok(
  $$insert into storage.objects (bucket_id, name)
    values ('mistake-assets',
            '22222222-2222-4222-8222-222222222222/sha256/image.enc')$$,
  '42501',
  'new row violates row-level security policy for table "objects"',
  'private storage rejects another account prefix'
);

select * from finish();
rollback;
