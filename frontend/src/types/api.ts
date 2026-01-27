export interface QuestionDetails {
  options?: string[];
  correct_answer?: string; // For multiple choice
  sample_input?: string; // For code
  sample_output?: string; // For code
  // Add other specific fields as needed based on QuestionType
}

export type QuestionType = 'multiple_choice' | 'text' | 'code' | 'short_answer';

export interface CreateQuestion {
  type: QuestionType;
  question: string;
  points: number;
  options?: string[]; // Flattened from details for easier usage if needed, or keep in details
  correct_answer?: string;
  min_words?: number;
  expected_keywords?: string[];
  _uid?: string; // Frontend animation key
  // ... map other details
}

export interface Vacancy {
  id: string;
  external_id?: string;
  title: string;
  company: string;
  location: string;
  employment_type?: string;
  salary_from?: number; // Decimal in Rust, number in TS
  salary_to?: number;
  currency?: string;
  negotiated_salary: boolean;
  description?: string;
  requirements?: string;
  responsibilities?: string;
  benefits?: string;
  apply_url?: string;
  contact_email?: string;
  contact_phone?: string;
  status: string;
  published_at?: string;
  created_at?: string;
  updated_at?: string;
}

export interface CreateVacancyPayload {
  title: string;
  company: string;
  location: string;
  employment_type?: string;
  salary_from?: number;
  salary_to?: number;
  currency?: string;
  negotiated_salary?: boolean;
  description?: string;
  requirements?: string;
  responsibilities?: string;
  benefits?: string;
  apply_url?: string;
  contact_email?: string;
  contact_phone?: string;
  status?: string;
}

export interface VacancyPublicSummary {
  id: string;
  title: string;
  company: string;
  location: string;
  employment_type?: string;
  salary_from?: number;
  salary_to?: number;
  currency?: string;
  negotiated_salary: boolean;
  summary?: string;
  published_at?: string;
}

export interface VacancyPublicListResponse {
  items: VacancyPublicSummary[];
}

export interface VacancyListResponse {
  items: Vacancy[];
}

export interface Test {
  id: string;
  title: string;
  description?: string;
  instructions?: string;
  duration_minutes: number;
  passing_score: number;
  questions?: any[];
  created_at?: string;
  is_active?: boolean;
  max_attempts?: number;
  shuffle_questions?: boolean;
  shuffle_options?: boolean;
  show_results_immediately?: boolean;
  test_type?: 'question_based' | 'presentation';
  presentation_themes?: string[];
  presentation_extra_info?: string;
}

export interface CreateTestPayload {
  title: string;
  description?: string;
  instructions?: string;
  questions?: CreateQuestion[];
  duration_minutes: number;
  passing_score: number;
  shuffle_questions?: boolean;
  shuffle_options?: boolean;
  show_results_immediately?: boolean;
  test_type?: 'question_based' | 'presentation';
  presentation_themes?: string[];
  presentation_extra_info?: string;
}

export interface Candidate {
  id: string;
  name: string;
  email: string;
  telegram_id?: number;
  phone?: string;
  cv_url?: string;
  dob?: string;
  vacancy_id?: number;
  profile_data?: any;
  ai_rating?: number;
  ai_comment?: string;
  created_at?: string;
  updated_at?: string;
}

export interface CreateInvitePayload {
  candidate: {
    name: string;
    email: string;
    telegram_id?: number;
    phone?: string;
    external_id?: string;
  };
  test_id: string;
  expires_in_hours?: number;
  send_notification?: boolean;
}

export interface PublicTestSummary {
  title: string;
  description?: string;
  instructions?: string;
  duration_minutes: number;
  total_questions: number;
  passing_score: number;
  test_type?: 'question_based' | 'presentation';
  presentation_themes?: string[];
  presentation_extra_info?: string;
}

export interface PublicAttemptSummary {
  id: string;
  status: string;
  expires_at: string;
  candidate_name: string;
}

export interface GetTestByTokenResponse {
  test: PublicTestSummary;
  attempt: PublicAttemptSummary;
}

export interface StartTestResponse {
  attempt_id: string;
  status: string;
  started_at: string;
  expires_at: string;
  questions: any; // Using any for now as it's a JSON value in Rust
}

export interface SaveAnswerResponse {
  saved: boolean;
  question_id: number;
  timestamp: string;
}

export interface SubmitTestRequest {
  answers: any[]; // Define more specifically if needed
}

export interface SubmitTestResponse {
  attempt_id: string;
  status: string;
  score: number;
  max_score: number;
  percentage: number;
  passed: boolean;
  show_results: boolean;
  message: string;
}

export interface ExternalVacancy {
  id: number;
  title: string;
  content: string; // HTML
  hot: boolean;
  city: string;
  direction: string;
  company_id: number | null;
  created_at: string;
}

export interface ExternalCompany {
  id: number;
  title: string;
  logo: string;
}

export interface ExternalVacancyListResponse {
  vacancies: ExternalVacancy[];
  companies: ExternalCompany[];
}

export interface CandidateApplication {
  id: number;
  candidate_id: string;
  vacancy_id: number;
  created_at: string;
}

export interface ApplyVacancyRequest {
  candidate_id: string;
  vacancy_id: number;
  /** Vacancy name for 1F integration (optional but recommended) */
  vacancy_name?: string;
}
