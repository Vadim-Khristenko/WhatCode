package space.vairice.whatcode

import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.ScrollView
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import java.io.File
import kotlin.concurrent.thread

/**
 * Простая и удобная UI-обёртка над нативным бинарём WhatCode.
 *
 * Бинарь поставляется в APK как `libwhatcode.so` (единственный способ держать
 * исполняемый файл в приложении: файлы из `nativeLibraryDir` можно запускать).
 * UI — чат: ввод сверху вниз, ответы модели в ленте. Каждый запрос выполняется
 * через одноразовый режим `whatcode --text`, вывод стримится в ленту.
 */
class MainActivity : AppCompatActivity() {

    private lateinit var output: TextView
    private lateinit var scroll: ScrollView
    private lateinit var binPath: String

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        output = findViewById(R.id.output)
        scroll = findViewById(R.id.scroll)
        val input = findViewById<EditText>(R.id.input)
        val send = findViewById<Button>(R.id.send)

        binPath = File(applicationInfo.nativeLibraryDir, "libwhatcode.so").absolutePath

        append("WhatCode ${BuildConfig.VERSION_NAME} — UI-обёртка\n")
        append("Персона: miku · движок: ${binPath}\n")
        append("Спроси что-нибудь. Для полных ответов задай LLM-провайдера через окружение (см. .env.example).\n\n")

        send.setOnClickListener {
            val prompt = input.text.toString().trim()
            if (prompt.isEmpty()) return@setOnClickListener
            input.text.clear()
            append("› $prompt\n")
            send.isEnabled = false
            thread {
                val reply = runWhatCode(prompt)
                runOnUiThread {
                    append("$reply\n\n")
                    send.isEnabled = true
                }
            }
        }
    }

    private fun append(text: String) {
        output.append(text)
        scroll.post { scroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    /** Запустить бинарь в одноразовом режиме и вернуть его вывод. */
    private fun runWhatCode(prompt: String): String {
        return try {
            val pb = ProcessBuilder(binPath, "--text", prompt)
            pb.redirectErrorStream(true)
            // Данные приложения — писабельный HOME для памяти/логов.
            pb.environment()["HOME"] = filesDir.absolutePath
            pb.environment().putIfAbsent("WHATCODE_PERSONA", "miku")
            pb.directory(filesDir)
            val proc = pb.start()
            val out = proc.inputStream.bufferedReader().readText()
            proc.waitFor()
            out.trim().ifEmpty { "(нет вывода)" }
        } catch (e: Exception) {
            "Ошибка запуска WhatCode: ${e.message}"
        }
    }
}
