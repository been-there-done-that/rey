"use client"

import { useState } from "react"
import { useForm } from "react-hook-form"
import { zodResolver } from "@hookform/resolvers/zod"
import { useRouter } from "next/navigation"
import { z } from "zod/v3"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { api } from "@/lib/api"
import { useAuth } from "@/lib/auth-store"
import { deriveKek, deriveVerificationKey } from "@/lib/auth-crypto"

const emailSchema = z.object({ email: z.string().email("Invalid email address") })
type EmailInput = z.infer<typeof emailSchema>

const passwordSchema = z.object({ password: z.string().min(1, "Password is required") })
type PasswordInput = z.infer<typeof passwordSchema>

export function LoginForm({
  className,
  ...props
}: React.ComponentProps<"div">) {
  const router = useRouter()
  const setToken = useAuth((s) => s.setToken)
  const [step, setStep] = useState<"email" | "password">("email")
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState("")
  const [email, setEmail] = useState("")
  const [loginParams, setLoginParams] = useState<{ kek_salt: string; mem_limit: number; ops_limit: number } | null>(null)

  const emailForm = useForm<EmailInput>({
    resolver: zodResolver(emailSchema),
    defaultValues: { email: "" },
  })

  const passwordForm = useForm<PasswordInput>({
    resolver: zodResolver(passwordSchema),
    defaultValues: { password: "" },
  })

  async function onEmailSubmit(data: EmailInput) {
    setLoading(true)
    setError("")

    try {
      const params = await api.post("api/auth/login-params", {
        json: { email: data.email },
      }).json<{ kek_salt: string; mem_limit: number; ops_limit: number }>()

      setEmail(data.email)
      setLoginParams(params)
      setStep("password")
    } catch {
      setError("Failed to look up account")
    } finally {
      setLoading(false)
    }
  }

  async function onPasswordSubmit(data: PasswordInput) {
    if (!loginParams) return

    setLoading(true)
    setError("")

    try {
      const kek = await deriveKek(data.password, loginParams.kek_salt)
      const verifyKey = await deriveVerificationKey(kek)

      const loginRes = await api.post("api/auth/login", {
        json: { email, verify_key_hash: verifyKey },
      }).json<{ session_token: string }>()

      setToken(loginRes.session_token)
      router.push("/d/home")
    } catch {
      setError("Invalid password")
    } finally {
      setLoading(false)
    }
  }

  function goBack() {
    setStep("email")
    setLoginParams(null)
    setError("")
    passwordForm.reset()
  }

  return (
    <div className={cn("flex flex-col gap-6", className)} {...props}>
      <Card>
        <CardHeader className="text-center">
          <CardTitle className="text-xl">
            {step === "email" ? "Welcome back" : "Enter your password"}
          </CardTitle>
          <CardDescription>
            {step === "email"
              ? "Enter your email to continue"
              : email}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {step === "email" ? (
            <form onSubmit={emailForm.handleSubmit(onEmailSubmit)}>
              <FieldGroup>
                {error && (
                  <div className="rounded-lg border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                    {error}
                  </div>
                )}
                <Field>
                  <FieldLabel htmlFor="email">Email</FieldLabel>
                  <Input
                    id="email"
                    type="email"
                    placeholder="m@example.com"
                    {...emailForm.register("email")}
                  />
                  <FieldError errors={emailForm.formState.errors.email ? [emailForm.formState.errors.email] : []} />
                </Field>
                <Field>
                  <Button type="submit" disabled={loading}>
                    {loading ? "Checking..." : "Continue"}
                  </Button>
                  <FieldDescription className="text-center">
                    Don&apos;t have an account?{" "}
                    <a href="/signup" className="underline underline-offset-4">
                      Sign up
                    </a>
                  </FieldDescription>
                </Field>
              </FieldGroup>
            </form>
          ) : (
            <form onSubmit={passwordForm.handleSubmit(onPasswordSubmit)}>
              <FieldGroup>
                {error && (
                  <div className="rounded-lg border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                    {error}
                  </div>
                )}
                <Field>
                  <FieldLabel htmlFor="password">Password</FieldLabel>
                  <Input
                    id="password"
                    type="password"
                    autoFocus
                    {...passwordForm.register("password")}
                  />
                  <FieldError errors={passwordForm.formState.errors.password ? [passwordForm.formState.errors.password] : []} />
                </Field>
                <Field>
                  <div className="flex gap-2">
                    <Button type="button" variant="outline" onClick={goBack} disabled={loading}>
                      Back
                    </Button>
                    <Button type="submit" disabled={loading} className="flex-1">
                      {loading ? "Logging in..." : "Login"}
                    </Button>
                  </div>
                </Field>
              </FieldGroup>
            </form>
          )}
        </CardContent>
      </Card>
      <FieldDescription className="px-6 text-center">
        By clicking continue, you agree to our{" "}
        <a href="#" className="underline underline-offset-4">
          Terms of Service
        </a>{" "}
        and{" "}
        <a href="#" className="underline underline-offset-4">
          Privacy Policy
        </a>
        .
      </FieldDescription>
    </div>
  )
}
