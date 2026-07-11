export type SimpleRating = 'forgot' | 'remembered'
export type FsrsRating = 'again' | 'hard' | 'good' | 'easy'

export function mapSimpleRating(rating: SimpleRating): FsrsRating {
  return rating === 'forgot' ? 'again' : 'good'
}
