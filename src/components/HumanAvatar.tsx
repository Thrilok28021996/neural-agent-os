import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'

type AssistantState = 'idle' | 'listening' | 'speaking' | 'thinking'

interface HumanAvatarProps {
  state: AssistantState
  size?: number
  onStateChange?: (state: AssistantState) => void
  /** Latest assistant reply, shown in a speech bubble while speaking. */
  speech?: string
}

interface SceneRefs {
  root: THREE.Group        // gentle sway of the whole figure
  body: THREE.Group        // breathing / bouncing
  headPivot: THREE.Group   // nod / shake / tilt
  face: THREE.Group
  eyes: THREE.Group[]      // blink + expression
  brows: THREE.Mesh[]
  nose: THREE.Mesh
  mouthCavity: THREE.Mesh  // opens while talking
  smile: THREE.Mesh        // happy arc
  blush: THREE.Mesh[]
  upperArmL: THREE.Group
  forearmL: THREE.Group
  handL: THREE.Mesh
  upperArmR: THREE.Group
  forearmR: THREE.Group
  handR: THREE.Mesh
  glow: THREE.Mesh
  particles: THREE.Points
  waistRing: THREE.Mesh
  chestLight: THREE.Mesh
}

const SKIN = 0xf2c9a0
const SKIN_DARK = 0xdfae84
const HAIR = 0x33241a
const SHIRT = 0x6f5bd6
const SHIRT_DARK = 0x5a49b8
const EYE_WHITE = 0xf7f7fb
const IRIS = 0x4f7fc9
const PUPIL = 0x0c0d12
const GLOW = 0x22d3ee

/**
 * A stylised but human-proportioned 3D avatar (head, neck, torso, two-joint
 * arms, hands) that reacts to the assistant's state:
 *   idle     – breathing, blinking, gentle sway
 *   listening– leans in, attentive eyes, slight smile
 *   speaking – mouth moves at speech cadence, nods, small gestures
 *   thinking – looks up, brow furrowed, hand resting at the chin
 */
export function HumanAvatar({ state, size = 280, onStateChange, speech }: HumanAvatarProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const sceneRef = useRef<SceneRefs | null>(null)
  const frameRef = useRef(0)
  const stateRef = useRef(state)

  useEffect(() => { stateRef.current = state }, [state])

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const scene = new THREE.Scene()
    const camera = new THREE.PerspectiveCamera(34, 1, 0.1, 100)
    camera.position.set(0, 1.07, 2.2)
    camera.lookAt(0, 1.07, 0)

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true })
    renderer.setSize(size, size)
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    container.appendChild(renderer.domElement)

    // ── Lighting ──────────────────────────────────────────────────────────
    scene.add(new THREE.AmbientLight(0xb9c0ff, 1.6))
    const keyLight = new THREE.DirectionalLight(0xfff1de, 2.2)
    keyLight.position.set(2.5, 4, 3)
    scene.add(keyLight)
    const fill = new THREE.DirectionalLight(0x9db4ff, 0.9)
    fill.position.set(-3, 1.5, 2)
    scene.add(fill)
    const rim = new THREE.DirectionalLight(0x22d3ee, 1.4)
    rim.position.set(-1.5, 2.5, -4)
    scene.add(rim)
    const under = new THREE.PointLight(0x7c6cff, 6, 6)
    under.position.set(0, 0.1, 1.6)
    scene.add(under)

    // ── Materials ─────────────────────────────────────────────────────────
    const skinMat = new THREE.MeshStandardMaterial({ color: SKIN, roughness: 0.62, metalness: 0.0 })
    const skinDarkMat = new THREE.MeshStandardMaterial({ color: SKIN_DARK, roughness: 0.7 })
    const hairMat = new THREE.MeshStandardMaterial({ color: HAIR, roughness: 0.85 })
    const shirtMat = new THREE.MeshStandardMaterial({ color: SHIRT, roughness: 0.55, metalness: 0.08 })
    const shirtDarkMat = new THREE.MeshStandardMaterial({ color: SHIRT_DARK, roughness: 0.6 })

    const root = new THREE.Group()
    scene.add(root)
    const body = new THREE.Group()
    root.add(body)

    // ── Torso (waist up, hologram-truncated) ─────────────────────────────
    const torso = new THREE.Mesh(new THREE.CapsuleGeometry(0.185, 0.5, 8, 24), shirtMat)
    torso.position.y = 0.9
    torso.scale.set(1, 1, 0.82)
    body.add(torso)
    const chest = new THREE.Mesh(new THREE.CapsuleGeometry(0.19, 0.22, 8, 24), shirtDarkMat)
    chest.position.set(0, 1.12, -0.01)
    chest.scale.set(0.96, 1, 0.8)
    body.add(chest)
    const collar = new THREE.Mesh(new THREE.TorusGeometry(0.09, 0.016, 10, 24), shirtDarkMat)
    collar.rotation.x = Math.PI / 2
    collar.position.y = 1.29
    body.add(collar)
    // glowing chest core (subtle heartbeat)
    const chestLight = new THREE.Mesh(new THREE.CircleGeometry(0.028, 20), new THREE.MeshBasicMaterial({ color: GLOW }))
    chestLight.position.set(0, 1.14, 0.155)
    body.add(chestLight)
    // waist truncation ring (hologram cut)
    const waistRing = new THREE.Mesh(
      new THREE.TorusGeometry(0.17, 0.008, 10, 40),
      new THREE.MeshBasicMaterial({ color: GLOW, transparent: true, opacity: 0.7 }),
    )
    waistRing.rotation.x = Math.PI / 2
    waistRing.position.y = 0.52
    body.add(waistRing)

    // ── Neck ──────────────────────────────────────────────────────────────
    const neck = new THREE.Mesh(new THREE.CapsuleGeometry(0.055, 0.08, 6, 16), skinMat)
    neck.position.y = 1.38
    body.add(neck)

    // ── Head ──────────────────────────────────────────────────────────────
    const headPivot = new THREE.Group()
    headPivot.position.y = 1.44
    body.add(headPivot)

    const head = new THREE.Group()
    headPivot.add(head)
    const skull = new THREE.Mesh(new THREE.SphereGeometry(0.165, 40, 28), skinMat)
    skull.scale.set(1, 1.08, 0.94)
    head.add(skull)

    // ears
    const earMat = skinMat
    const earL = new THREE.Mesh(new THREE.SphereGeometry(0.028, 14, 10), earMat)
    earL.position.set(-0.158, 0.03, 0)
    earL.scale.set(1, 1.25, 0.6)
    const earR = earL.clone()
    earR.position.x = 0.158
    head.add(earL, earR)

    // hair – cap + fringe (short modern crop)
    const hairCap = new THREE.Mesh(
      new THREE.SphereGeometry(0.172, 32, 20, 0, Math.PI * 2, 0, Math.PI * 0.58),
      hairMat,
    )
    hairCap.position.y = 0.04
    head.add(hairCap)
    const fringeGeo = new THREE.BoxGeometry(0.16, 0.05, 0.07)
    const fringe = new THREE.Mesh(fringeGeo, hairMat)
    fringe.position.set(0, 0.1, 0.145)
    fringe.rotation.x = -0.12
    head.add(fringe)
    const sideL = new THREE.Mesh(new THREE.BoxGeometry(0.03, 0.1, 0.14), hairMat)
    sideL.position.set(-0.145, 0.02, -0.01)
    sideL.rotation.z = 0.25
    const sideR = sideL.clone()
    sideR.position.x = 0.145
    sideR.rotation.z = -0.25
    head.add(sideL, sideR)

    // ── Face ──────────────────────────────────────────────────────────────
    const face = new THREE.Group()
    face.position.z = 0.13
    head.add(face)

    const eyeWhiteMat = new THREE.MeshStandardMaterial({ color: EYE_WHITE, roughness: 0.3 })
    const irisMat = new THREE.MeshStandardMaterial({ color: IRIS, roughness: 0.25, emissive: 0x102038, emissiveIntensity: 0.15 })
    const pupilMat = new THREE.MeshBasicMaterial({ color: PUPIL })
    const makeEye = (x: number) => {
      const eye = new THREE.Group()
      const sclera = new THREE.Mesh(new THREE.SphereGeometry(0.03, 20, 16), eyeWhiteMat)
      sclera.scale.set(1, 1.12, 0.55)
      const iris = new THREE.Mesh(new THREE.SphereGeometry(0.017, 16, 12), irisMat)
      iris.position.z = 0.024
      const pupil = new THREE.Mesh(new THREE.SphereGeometry(0.0085, 12, 10), pupilMat)
      pupil.position.z = 0.033
      eye.add(sclera, iris, pupil)
      eye.position.set(x, 0.055, 0.035)
      face.add(eye)
      return eye
    }
    const eyeL = makeEye(-0.065)
    const eyeR = makeEye(0.065)

    const browGeo = new THREE.BoxGeometry(0.055, 0.012, 0.014)
    const browMat = new THREE.MeshStandardMaterial({ color: HAIR, roughness: 0.9 })
    const browL = new THREE.Mesh(browGeo, browMat)
    browL.position.set(-0.065, 0.12, 0.045)
    browL.rotation.z = 0.12
    const browR = new THREE.Mesh(browGeo, browMat)
    browR.position.set(0.065, 0.12, 0.045)
    browR.rotation.z = -0.12
    face.add(browL, browR)

    const nose = new THREE.Mesh(new THREE.ConeGeometry(0.016, 0.05, 10), skinDarkMat)
    nose.rotation.x = Math.PI / 2
    nose.position.set(0, 0.005, 0.05)
    face.add(nose)

    // mouth: cavity (opens) + upper/lower lip arcs
    const mouthCavity = new THREE.Mesh(new THREE.BoxGeometry(0.055, 0.014, 0.008), new THREE.MeshBasicMaterial({ color: 0x4a2530 }))
    mouthCavity.position.set(0, -0.075, 0.035)
    face.add(mouthCavity)
    const lipMat = skinDarkMat
    const upperLip = new THREE.Mesh(new THREE.TorusGeometry(0.028, 0.006, 8, 18, Math.PI), lipMat)
    upperLip.rotation.z = Math.PI
    upperLip.position.set(0, -0.068, 0.04)
    face.add(upperLip)
    const lowerLip = new THREE.Mesh(new THREE.TorusGeometry(0.028, 0.0055, 8, 18, Math.PI), lipMat)
    lowerLip.position.set(0, -0.082, 0.04)
    face.add(lowerLip)
    // smile arc (happy) – hidden unless smiling
    const smile = new THREE.Mesh(new THREE.TorusGeometry(0.032, 0.006, 8, 18, Math.PI), new THREE.MeshBasicMaterial({ color: 0xd98a8a }))
    smile.rotation.z = Math.PI
    smile.position.set(0, -0.078, 0.041)
    smile.visible = false
    face.add(smile)

    const blushMat = new THREE.MeshBasicMaterial({ color: 0xff8fa3, transparent: true, opacity: 0.55 })
    const blushGeo = new THREE.SphereGeometry(0.014, 12, 10)
    const blushL = new THREE.Mesh(blushGeo, blushMat)
    blushL.position.set(-0.085, -0.03, 0.04)
    const blushR = new THREE.Mesh(blushGeo, blushMat)
    blushR.position.set(0.085, -0.03, 0.04)
    blushL.visible = blushR.visible = false
    face.add(blushL, blushR)

    // ── Arms (two joints: shoulder → elbow → hand) ────────────────────────
    const makeArm = (side: 1 | -1) => {
      const upper = new THREE.Group()
      upper.position.set(0.235 * side, 1.28, 0)
      const upperMesh = new THREE.Mesh(new THREE.CapsuleGeometry(0.052, 0.24, 6, 16), shirtMat)
      upperMesh.position.y = -0.16
      upper.add(upperMesh)
      const forearm = new THREE.Group()
      forearm.position.y = -0.32
      upper.add(forearm)
      const forearmMesh = new THREE.Mesh(new THREE.CapsuleGeometry(0.042, 0.22, 6, 16), skinMat)
      forearmMesh.position.y = -0.15
      forearm.add(forearmMesh)
      const hand = new THREE.Mesh(new THREE.SphereGeometry(0.055, 16, 12), skinMat)
      hand.position.y = -0.32
      hand.scale.set(1, 1.15, 0.75)
      forearm.add(hand)
      body.add(upper)
      return { upper, forearm, hand }
    }
    const armL = makeArm(-1)
    const armR = makeArm(1)

    // ── Hologram floor, glow & particles ──────────────────────────────────
    const disc = new THREE.Mesh(
      new THREE.CircleGeometry(0.72, 48),
      new THREE.MeshBasicMaterial({ color: 0x6f5bd6, transparent: true, opacity: 0.16, side: THREE.DoubleSide }),
    )
    disc.rotation.x = -Math.PI / 2
    disc.position.y = 0.02
    scene.add(disc)

    const glow = new THREE.Mesh(
      new THREE.SphereGeometry(1.05, 32, 32),
      new THREE.MeshBasicMaterial({ color: 0x8f7cff, transparent: true, opacity: 0.07 }),
    )
    glow.position.y = 1.0
    scene.add(glow)

    const particlesGeo = new THREE.BufferGeometry()
    const count = 140
    const pos = new Float32Array(count * 3)
    for (let i = 0; i < count * 3; i += 3) {
      const r = 0.9 + Math.random() * 0.9
      const th = Math.random() * Math.PI * 2
      const ph = Math.random() * Math.PI * 0.85
      pos[i] = r * Math.sin(ph) * Math.cos(th)
      pos[i + 1] = 0.55 + r * Math.cos(ph) * 0.8
      pos[i + 2] = r * Math.sin(ph) * Math.sin(th) * 0.7
    }
    particlesGeo.setAttribute('position', new THREE.BufferAttribute(pos, 3))
    const particles = new THREE.Points(
      particlesGeo,
      new THREE.PointsMaterial({ color: 0x9db4ff, size: 0.028, transparent: true, opacity: 0.65 }),
    )
    scene.add(particles)

    sceneRef.current = {
      root, body, headPivot, face, eyes: [eyeL, eyeR], brows: [browL, browR], nose,
      mouthCavity, smile, blush: [blushL, blushR],
      upperArmL: armL.upper, forearmL: armL.forearm, handL: armL.hand,
      upperArmR: armR.upper, forearmR: armR.forearm, handR: armR.hand,
      glow, particles, waistRing, chestLight,
    }

    // ── Animation loop ────────────────────────────────────────────────────
    let blinkAt = 2.5 + Math.random() * 2
    const animate = () => {
      frameRef.current = requestAnimationFrame(animate)
      const t = Date.now() * 0.001
      const refs = sceneRef.current
      if (!refs) return
      const s = stateRef.current

      // idle sway + breathing base
      refs.root.rotation.y = Math.sin(t * 0.45) * 0.14
      refs.body.rotation.z = Math.sin(t * 0.4) * 0.012
      const breathe = Math.sin(t * 1.7) * 0.016
      refs.body.scale.y = 1 + breathe
      refs.body.scale.x = 1 - breathe * 0.6

      // reset pose
      refs.headPivot.rotation.set(0, 0, 0)
      refs.face.rotation.set(0, 0, 0)
      refs.upperArmL.rotation.set(0, 0, 0)
      refs.forearmL.rotation.set(0, 0, 0)
      refs.upperArmR.rotation.set(0, 0, 0)
      refs.forearmR.rotation.set(0, 0, 0)
      refs.nose.rotation.x = Math.PI / 2
      refs.mouthCavity.scale.set(1, 1, 1)
      refs.smile.visible = false
      refs.blush.forEach((b) => (b.visible = false))
      for (const eye of refs.eyes) {
        eye.scale.set(1, 1, 1)
        eye.position.y = 0.055
      }
      refs.brows[0].rotation.z = 0.12
      refs.brows[1].rotation.z = -0.12
      refs.brows[0].position.y = 0.12
      refs.brows[1].position.y = 0.12

      // chest heartbeat + waist ring pulse
      refs.chestLight.scale.setScalar(1 + Math.sin(t * 2.2) * 0.18)
      ;(refs.chestLight.material as THREE.MeshBasicMaterial).opacity = 0.7 + Math.sin(t * 2.2) * 0.3
      refs.waistRing.scale.setScalar(1 + Math.sin(t * 1.7) * 0.03)
      refs.glow.scale.setScalar(1 + Math.sin(t * 0.8) * 0.05)
      refs.particles.rotation.y = t * 0.05

      // blink
      if (t > blinkAt) {
        blinkAt = t + 2.2 + Math.random() * 2.6
      }
      const since = blinkAt - t
      if (since > 0 && since < 0.13) {
        for (const eye of refs.eyes) eye.scale.y = 0.12
      }

      switch (s) {
        case 'listening': {
          // lean in, attentive
          refs.body.rotation.x = 0.07
          refs.headPivot.rotation.x = -0.08
          refs.headPivot.rotation.z = Math.sin(t * 1.1) * 0.04
          for (const eye of refs.eyes) {
            eye.scale.set(1.06, 1.12, 1)
          }
          refs.upperArmL.rotation.z = 0.05
          refs.upperArmR.rotation.z = -0.05
          refs.mouthCavity.scale.set(1, 0.4, 1)
          refs.smile.visible = true
          refs.blush.forEach((b) => (b.visible = true))
          break
        }
        case 'thinking': {
          // look up-left, furrowed brow, right hand rests at chin
          refs.headPivot.rotation.x = -0.16
          refs.headPivot.rotation.z = 0.09
          refs.face.rotation.z = 0.04
          for (const eye of refs.eyes) {
            eye.position.y = 0.068
            eye.scale.set(0.92, 1.02, 1)
          }
          refs.brows[0].rotation.z = 0.32
          refs.brows[1].rotation.z = -0.38
          refs.brows[0].position.y = 0.125
          refs.brows[1].position.y = 0.128
          refs.mouthCavity.scale.set(0.7, 0.55, 1)
          refs.upperArmR.rotation.x = 1.0
          refs.forearmR.rotation.x = -1.9
          refs.upperArmL.rotation.z = 0.12
          refs.upperArmL.rotation.x = 0.25
          break
        }
        case 'speaking': {
          // mouth cadence + nod + small excited gestures
          const talk = Math.abs(Math.sin(t * 8.5)) * 0.75 + 0.3
          refs.mouthCavity.scale.y = talk
          refs.mouthCavity.scale.x = 1 + talk * 0.35
          refs.headPivot.rotation.x = Math.sin(t * 2.1) * 0.05
          refs.headPivot.rotation.z = Math.sin(t * 1.4) * 0.03
          refs.smile.visible = true
          refs.upperArmL.rotation.z = Math.sin(t * 1.6) * 0.14
          refs.upperArmR.rotation.z = -Math.sin(t * 1.6) * 0.14
          refs.forearmL.rotation.x = Math.sin(t * 1.6) * 0.12
          refs.forearmR.rotation.x = -Math.sin(t * 1.6) * 0.12
          refs.blush.forEach((b) => (b.visible = true))
          break
        }
        default: {
          // idle: arms hang with a gentle sway
          refs.upperArmL.rotation.z = Math.sin(t * 0.9) * 0.05
          refs.upperArmR.rotation.z = -Math.sin(t * 0.9) * 0.05
          refs.forearmL.rotation.x = Math.sin(t * 0.9) * 0.03
          refs.forearmR.rotation.x = -Math.sin(t * 0.9) * 0.03
        }
      }

      renderer.render(scene, camera)
    }
    animate()

    return () => {
      cancelAnimationFrame(frameRef.current)
      renderer.dispose()
      sceneRef.current = null
      if (container.contains(renderer.domElement)) container.removeChild(renderer.domElement)
    }
  }, [size])

  const speaking = state === 'speaking'

  return (
    <div style={{ position: 'relative' }}>
      {speaking && speech && (
        <div className="three-d-speech" role="status" aria-live="polite">{speech}</div>
      )}
      <div
        ref={containerRef}
        style={{ width: size, height: size, cursor: 'pointer' }}
        onClick={() => onStateChange?.(state === 'idle' ? 'listening' : 'idle')}
        title={`Assistant state: ${state}${speaking ? ' · speaking' : ''}`}
      />
    </div>
  )
}
